[CmdletBinding()]
param(
    [ValidateSet("win-x64")]
    [string]$Runtime = "win-x64",
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\..\..\generated-artifacts\windows")
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw "Build this Windows desktop application on Windows with PowerShell 7 and .NET 9 SDK." }
$root = Split-Path $PSScriptRoot -Parent
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
$staging = Join-Path $output "Throw-Launcher-$Runtime"
if (Test-Path $staging) {
    throw "Output already exists: $staging. Choose a new OutputDirectory; existing builds are never deleted automatically."
}
New-Item -ItemType Directory -Force -Path $output | Out-Null
Push-Location $root
try {
    dotnet build Flow.Launcher.sln -c Release -p:SkipLauncherTermination=true --nologo
    if ($LASTEXITCODE -ne 0) { throw "Solution build failed ($LASTEXITCODE)." }
    dotnet publish Flow.Launcher/Flow.Launcher.csproj -c Release -r $Runtime --self-contained true -p:SkipLauncherTermination=true -o $staging --nologo
    if ($LASTEXITCODE -ne 0) { throw "Windows publish failed ($LASTEXITCODE)." }
    Copy-Item Output/Release/Plugins (Join-Path $staging "Plugins") -Recurse -Force
    # An empty UserData directory selects portable mode. No installed profile
    # or application files are modified by this script.
    New-Item -ItemType Directory (Join-Path $staging "UserData") | Out-Null
    Copy-Item LITE_MODE.md $staging
    $archive = "$staging.zip"
    Compress-Archive -Path $staging -DestinationPath $archive
    Write-Output "Windows test package: $archive"
    Write-Output "This is a test build, not a signed installer or a measured performance claim."
}
finally { Pop-Location }
