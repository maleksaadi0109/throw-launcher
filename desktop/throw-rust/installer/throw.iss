; Per-user, offline installer. No admin rights, network downloads or bundled runtime.
#ifndef BuildVersion
  #define BuildVersion "0.1.0"
#endif
[Setup]
AppId={{4AD9D355-7608-4FB0-920C-733EF323784B}
AppName=Throw Launcher
AppVersion={#BuildVersion}
AppPublisher=Throw Launcher
AppPublisherURL=https://github.com/maleksaadi0109/throw-launcher
AppSupportURL=https://github.com/maleksaadi0109/throw-launcher/issues
DefaultDirName={localappdata}\Programs\Throw Launcher
DefaultGroupName=Throw Launcher
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
WizardStyle=modern
WizardSizePercent=120
WizardResizable=yes
DisableWelcomePage=no
DisableDirPage=no
DisableProgramGroupPage=yes
SetupIconFile=..\branding\throw-rust.ico
WizardImageFile=..\..\..\generated-artifacts\throw-rust\setup-sidebar.bmp
WizardSmallImageFile=..\..\..\generated-artifacts\throw-rust\setup-small.bmp
UninstallDisplayIcon={app}\Throw.Rust.exe
AppMutex=Local\ThrowLauncherRust
CloseApplications=yes
RestartApplications=no
Compression=lzma2
SolidCompression=yes
OutputDir=..\..\..\generated-artifacts\throw-rust
OutputBaseFilename=Throw-Launcher-Setup-x64
LicenseFile=..\..\..\LICENSE

[Messages]
WelcomeLabel1=Welcome to Throw
WelcomeLabel2=A quieter way to open apps, find files, and get things done.%n%nSetup will install Throw Launcher for your Windows account. You can choose where it lives, whether it starts with Windows, and how it looks.%n%nNo administrator access or additional runtime is needed.
FinishedHeadingLabel=Throw is ready
FinishedLabel=Throw Launcher is installed.%n%nUse your chosen keyboard shortcut to open it. You can change appearance, startup and shortcuts in Settings at any time.%n%nTo fully close Throw, choose Quit from its tray menu.

[Tasks]
Name: "startup"; Description: "Start Throw when I sign in to Windows"; GroupDescription: "Everyday use:"; Flags: unchecked
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "..\..\..\generated-artifacts\throw-rust\package\Throw.Rust.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\README.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\..\LICENSE"; DestDir: "{app}"; DestName: "LICENSE.txt"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Throw Launcher"; Filename: "{app}\Throw.Rust.exe"
Name: "{autodesktop}\Throw Launcher"; Filename: "{app}\Throw.Rust.exe"; Tasks: desktopicon

[InstallDelete]
Type: files; Name: "{autodesktop}\Throw Launcher.lnk"; Tasks: not desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "ThrowLauncherRust"; ValueData: """{app}\Throw.Rust.exe"" --background"; Tasks: startup
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueName: "ThrowLauncherRust"; Flags: deletevalue; Tasks: not startup

[Run]
Filename: "{app}\Throw.Rust.exe"; Description: "Open Throw Launcher"; Flags: nowait postinstall skipifsilent; Check: CanLaunch

[Code]
var
  PreferencesPage: TWizardPage;
  Customize, Lite, HideOnBlur, History: TNewCheckBox;
  ThemeChoice, ShortcutChoice: TNewComboBox;
  ThemeLabel, ShortcutLabel: TNewStaticText;
  StartupPageInitialized: Boolean;

function CheckBoxOn(Page: TWizardPage; Caption: String; Top: Integer): TNewCheckBox;
begin
  Result := TNewCheckBox.Create(WizardForm);
  Result.Parent := Page.Surface;
  Result.Caption := Caption;
  Result.SetBounds(0, ScaleY(Top), Page.SurfaceWidth, ScaleY(24));
end;

procedure UpdatePreferences(Sender: TObject);
begin
  ThemeChoice.Enabled := Customize.Checked;
  ShortcutChoice.Enabled := Customize.Checked;
  Lite.Enabled := Customize.Checked;
  HideOnBlur.Enabled := Customize.Checked;
  History.Enabled := Customize.Checked;
  ThemeLabel.Enabled := Customize.Checked;
  ShortcutLabel.Enabled := Customize.Checked;
end;

procedure InitializeWizard;
begin
  PreferencesPage := CreateCustomPage(wpSelectTasks, 'Make Throw yours',
    'A few choices for your first launch. Everything can be changed later in Settings.');
  Customize := CheckBoxOn(PreferencesPage, 'Apply the preferences below (otherwise keep my existing setup)', 0);
  Customize.Checked := not FileExists(ExpandConstant('{userappdata}\ThrowLauncherRust\settings.json'));
  Customize.OnClick := @UpdatePreferences;
  ThemeLabel := TNewStaticText.Create(WizardForm);
  ThemeLabel.Parent := PreferencesPage.Surface;
  ThemeLabel.Caption := 'Appearance';
  ThemeLabel.SetBounds(0, ScaleY(42), ScaleX(240), ScaleY(20));
  ThemeChoice := TNewComboBox.Create(WizardForm);
  ThemeChoice.Parent := PreferencesPage.Surface;
  ThemeChoice.Style := csDropDownList;
  ThemeChoice.SetBounds(0, ScaleY(64), ScaleX(300), ScaleY(26));
  ThemeChoice.Items.Add('Pearl - light');
  ThemeChoice.Items.Add('Graphite - dark');
  ThemeChoice.ItemIndex := 0;
  ShortcutLabel := TNewStaticText.Create(WizardForm);
  ShortcutLabel.Parent := PreferencesPage.Surface;
  ShortcutLabel.Caption := 'Open Throw with';
  ShortcutLabel.SetBounds(0, ScaleY(102), ScaleX(240), ScaleY(20));
  ShortcutChoice := TNewComboBox.Create(WizardForm);
  ShortcutChoice.Parent := PreferencesPage.Surface;
  ShortcutChoice.Style := csDropDownList;
  ShortcutChoice.SetBounds(0, ScaleY(124), ScaleX(300), ScaleY(26));
  ShortcutChoice.Items.Add('Ctrl + Alt + Space (recommended)');
  ShortcutChoice.Items.Add('Alt + Space');
  ShortcutChoice.Items.Add('Ctrl + Shift + Space');
  ShortcutChoice.ItemIndex := 0;
  Lite := CheckBoxOn(PreferencesPage, 'Use Lite mode - smaller bounded caches (recommended)', 170);
  Lite.Checked := True;
  HideOnBlur := CheckBoxOn(PreferencesPage, 'Hide the launcher when I switch to another window', 198);
  HideOnBlur.Checked := True;
  History := CheckBoxOn(PreferencesPage, 'Remember launch history on this device for better ranking', 226);
  History.Checked := True;
  UpdatePreferences(nil);
end;

procedure CurPageChanged(CurPageID: Integer);
var Tasks: String;
begin
  if (CurPageID = wpSelectTasks) and not StartupPageInitialized and not WizardSilent and
    (ExpandConstant('{param:TASKS|__unset__}') = '__unset__') and
    (ExpandConstant('{param:MERGETASKS|__unset__}') = '__unset__') then begin
    StartupPageInitialized := True;
    Tasks := WizardSelectedTasks(False);
    if RegValueExists(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'ThrowLauncherRust') then
      WizardSelectTasks(Tasks + ',startup')
    else
      WizardSelectTasks(Tasks + ',!startup');
  end;
end;

function CanLaunch: Boolean;
begin
  Result := ExpandConstant('{param:NOLAUNCH|0}') <> '1';
end;

function BooleanText(Value: Boolean): String;
begin
  if Value then Result := 'true' else Result := 'false';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var Params, Theme, Shortcut: String; ExitCode: Integer;
begin
  if (CurStep = ssPostInstall) and Customize.Checked then begin
    Theme := 'light';
    if ThemeChoice.ItemIndex = 1 then Theme := 'ink';
    Shortcut := 'ctrl-alt-space';
    if ShortcutChoice.ItemIndex = 1 then Shortcut := 'alt-space';
    if ShortcutChoice.ItemIndex = 2 then Shortcut := 'ctrl-shift-space';
    Params := '--configure-install --theme=' + Theme + ' --hotkey=' + Shortcut +
      ' --lite=' + BooleanText(Lite.Checked) +
      ' --hide-on-focus-loss=' + BooleanText(HideOnBlur.Checked) +
      ' --history=' + BooleanText(History.Checked);
    if not Exec(ExpandConstant('{app}\Throw.Rust.exe'), Params, ExpandConstant('{app}'),
      SW_HIDE, ewWaitUntilTerminated, ExitCode) then
      RaiseException('Setup could not start Throw to save your preferences. Existing preferences have not been replaced.');
    if ExitCode <> 0 then
      RaiseException('Throw could not save setup preferences. Quit any running Throw instance and check your existing settings file. Setup will not replace a damaged profile.');
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var Command: String;
begin
  if CurUninstallStep = usUninstall then begin
    if RegQueryStringValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run',
      'ThrowLauncherRust', Command) then
      if CompareText(Command, '"' + ExpandConstant('{app}\Throw.Rust.exe') + '" --background') = 0 then
        RegDeleteValue(HKCU, 'Software\Microsoft\Windows\CurrentVersion\Run', 'ThrowLauncherRust');
    { Keep the user's profile and history. Never remove another edition's data. }
  end;
end;
