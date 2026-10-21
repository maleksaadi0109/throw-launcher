# Runs only on a disposable Windows CI runner, never against a user's profile.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Installer lifecycle test requires a disposable GitHub Actions runner.' }
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class InstallerWindow {
    public delegate bool EnumProc(IntPtr hwnd, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr data);
    [DllImport("user32.dll")] public static extern bool EnumChildWindows(IntPtr parent, EnumProc callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr hwnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hwnd, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hwnd, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
    public struct Rect { public int Left,Top,Right,Bottom; }
}
'@
$output = [IO.Path]::GetFullPath("$PSScriptRoot/../../../generated-artifacts/throw-rust")
$setup = Join-Path $output 'Throw-Launcher-Setup-x64.exe'
$installDir = Join-Path $env:LOCALAPPDATA 'Throw Installer CI'
$profile = Join-Path $env:APPDATA 'ThrowLauncherRust'
$runKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
$startupName = 'ThrowLauncherRust'
function Startup-Value {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Software\Microsoft\Windows\CurrentVersion\Run')
    if ($null -eq $key) { return $null }
    try { return $key.GetValue($startupName, $null) } finally { $key.Dispose() }
}
if (Test-Path $profile) { throw 'Refusing to touch a pre-existing Throw profile.' }
if (Startup-Value) { throw 'Refusing to replace an existing startup entry.' }

function Children([IntPtr]$window) {
    $items = [Collections.Generic.List[object]]::new()
    $callback = [InstallerWindow+EnumProc]{
        param($handle, $data)
        if ([InstallerWindow]::IsWindowVisible($handle)) {
            $text = [Text.StringBuilder]::new(512)
            $class = [Text.StringBuilder]::new(128)
            [void][InstallerWindow]::GetWindowText($handle,$text,512)
            [void][InstallerWindow]::GetClassName($handle,$class,128)
            $items.Add([PSCustomObject]@{ Handle=$handle; Text=$text.ToString().Replace('&',''); Class=$class.ToString() })
        }
        return $true
    }
    [void][InstallerWindow]::EnumChildWindows($window,$callback,[IntPtr]::Zero)
    return $items
}
function Wait-Until([scriptblock]$check, [string]$message) {
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    do {
        if (& $check) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw $message
}
function Find-Wizard {
    $found = [Collections.Generic.List[IntPtr]]::new()
    $callback = [InstallerWindow+EnumProc]{
        param($handle, $data)
        $class = [Text.StringBuilder]::new(128)
        $text = [Text.StringBuilder]::new(512)
        [void][InstallerWindow]::GetClassName($handle,$class,128)
        [void][InstallerWindow]::GetWindowText($handle,$text,512)
        if ([InstallerWindow]::IsWindowVisible($handle) -and
            $class.ToString() -eq 'TWizardForm' -and $text.ToString().Contains('Throw')) {
            $found.Add($handle)
        }
        return $true
    }
    [void][InstallerWindow]::EnumWindows($callback,[IntPtr]::Zero)
    if ($found.Count -gt 0) { return $found[0] }
    return [IntPtr]::Zero
}
function Click([string]$caption) {
    $control = @(Children $script:window | Where-Object {
        $_.Text.Trim().TrimEnd('>').Trim() -eq $caption.Trim().TrimEnd('>').Trim() -and
            [InstallerWindow]::IsWindowEnabled($_.Handle)
    }) | Select-Object -First 1
    if (-not $control) { throw "Missing enabled installer control '$caption'. Visible: $((Children $script:window).Text -join ' | ')" }
    [void][InstallerWindow]::SetForegroundWindow($script:window)
    [void][InstallerWindow]::PostMessage($control.Handle,0xF5,[IntPtr]::Zero,[IntPtr]::Zero)
    Start-Sleep -Milliseconds 200
}
function Wait-Text([string]$text) {
    Wait-Until { @(Children $script:window | Where-Object { $_.Text.Contains($text) }).Count -gt 0 } "Installer did not show '$text'."
}
function Capture([string]$name) {
    $rect = [InstallerWindow+Rect]::new()
    [void][InstallerWindow]::GetWindowRect($script:window,[ref]$rect)
    $bmp = [Drawing.Bitmap]::new($rect.Right-$rect.Left,$rect.Bottom-$rect.Top)
    $g = [Drawing.Graphics]::FromImage($bmp)
    $dc = $g.GetHdc()
    try {
        if (-not [InstallerWindow]::PrintWindow($script:window,$dc,2)) { throw "Cannot capture $name" }
    } finally { $g.ReleaseHdc($dc); $g.Dispose() }
    try { $bmp.Save("$output/screenshots/$name.png",[Drawing.Imaging.ImageFormat]::Png) }
    finally { $bmp.Dispose() }
}
function Run-And-Wait([string]$exe, [string[]]$arguments) {
    $p = Start-Process $exe -ArgumentList $arguments -PassThru
    if (-not $p.WaitForExit(60000)) { $p.Kill(); throw "$exe timed out" }
    if ($p.ExitCode -ne 0) { throw "$exe exited with $($p.ExitCode)" }
}
$process = $null
try {
    # Actual visible first-time wizard, not just a silent smoke install.
    $process = Start-Process $setup -ArgumentList @("/DIR=`"$installDir`"",'/NOLAUNCH=1','/TASKS=startup,desktopicon',"/LOG=`"$output/install.log`"") -PassThru
    # Inno's bootstrapper may own no visible window; its temporary child does.
    Wait-Until { $script:window = Find-Wizard; $script:window -ne [IntPtr]::Zero } 'Installer did not create a window.'
    Wait-Text 'Welcome to Throw'
    Capture 'throw-installer-welcome'
    Click 'Next >'
    Wait-Text 'I accept the agreement'
    Click 'I accept the agreement'
    Click 'Next >'
    Wait-Text 'Select Destination Location'
    Capture 'throw-installer-location'
    Click 'Next >'
    Wait-Text 'Select Additional Tasks'
    Capture 'throw-installer-startup'
    Click 'Next >'
    Wait-Text 'Make Throw yours'
    $combos = @(Children $script:window | Where-Object { $_.Class -eq 'TNewComboBox' })
    if ($combos.Count -ne 2) { throw 'Missing theme or shortcut choices.' }
    foreach ($combo in $combos) {
        [void][InstallerWindow]::SendMessage($combo.Handle,0x14E,[IntPtr]1,[IntPtr]::Zero) # CB_SETCURSEL
    }
    Click 'Remember launch history on this device for better ranking'
    Capture 'throw-installer-preferences'
    Click 'Next >'
    Wait-Text 'Ready to Install'
    Click 'Install'
    Wait-Text 'Throw is ready'
    Capture 'throw-installer-finished'
    Click 'Finish'
    if (-not $process.WaitForExit(15000) -or $process.ExitCode -ne 0) { throw 'Interactive installer failed.' }
    $settingsPath = Join-Path $profile 'settings.json'
    $settings = Get-Content $settingsPath -Raw | ConvertFrom-Json
    if ($settings.theme -ne 'Ink' -or $settings.hotkey -ne 'AltSpace' -or $settings.remember_history -ne $false -or -not $settings.lite) {
        throw 'First-run wizard choices were not saved.'
    }
    $installedExe = Join-Path $installDir 'Throw.Rust.exe'
    $expectedStartup = '"' + $installedExe + '" --background'
    if ((Startup-Value) -ne $expectedStartup) { throw 'Startup choice did not register the installed executable.' }
    if (-not (Test-Path "$([Environment]::GetFolderPath('Programs'))/Throw Launcher.lnk")) { throw 'Start menu shortcut missing.' }
    if (-not (Test-Path "$([Environment]::GetFolderPath('Desktop'))/Throw Launcher.lnk")) { throw 'Desktop shortcut missing.' }
    $before = [IO.File]::ReadAllBytes($settingsPath)
    # Upgrade preserves exact preferences while an explicit task choice disables startup.
    Run-And-Wait $setup @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART',"/DIR=`"$installDir`"",'/TASKS=!startup,!desktopicon',"/LOG=`"$output/upgrade.log`"")
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($settingsPath)) -ne [Convert]::ToBase64String($before)) { throw 'Upgrade overwrote existing preferences.' }
    if (Startup-Value) { throw 'Startup was not disabled.' }
    # Enabling startup later from Settings must also be cleaned by uninstallation.
    Set-ItemProperty $runKey $startupName $expectedStartup
    Run-And-Wait (Join-Path $installDir 'unins000.exe') @('/VERYSILENT','/SUPPRESSMSGBOXES','/NORESTART')
    if (Test-Path $installedExe) { throw 'Uninstaller left the installed binary.' }
    if (Startup-Value) { throw 'Uninstaller left the installed startup entry.' }
    if (Test-Path "$([Environment]::GetFolderPath('Programs'))/Throw Launcher.lnk") { throw 'Uninstaller left the Start menu shortcut.' }
    if ([Convert]::ToBase64String([IO.File]::ReadAllBytes($settingsPath)) -ne [Convert]::ToBase64String($before)) { throw 'Uninstaller removed or changed personal preferences.' }
    'PASS: visible wizard, chosen theme/shortcut/privacy, startup, shortcuts, upgrade preservation, uninstall and retained profile.' |
        Set-Content "$output/installer-test-result.txt"
} finally {
    if ($process -and -not $process.HasExited) { $process.Kill() }
    # Never delete preferences here: retain evidence when a check fails.
}
