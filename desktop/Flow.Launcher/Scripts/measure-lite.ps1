[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidateRange(1, 2147483647)]
    [int]$LauncherProcessId,
    [ValidateSet("normal", "lite")]
    [string]$Mode = "lite",
    [ValidateRange(5, 600)]
    [int]$Seconds = 60,
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\..\..\generated-artifacts\measurements")
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw "Run on Windows with PowerShell 7." }
Get-Process -Id $LauncherProcessId -ErrorAction Stop | Out-Null
$samples = for ($i = 0; $i -lt $Seconds; $i++) {
    $all = @(Get-CimInstance Win32_Process)
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    $ids.Add($LauncherProcessId) | Out-Null
    do {
        $changed = $false
        foreach ($process in $all) {
            if ($ids.Contains([int]$process.ParentProcessId)) {
                if ($ids.Add([int]$process.ProcessId)) { $changed = $true }
            }
        }
    } while ($changed)
    $root = Get-Process -Id $LauncherProcessId -ErrorAction Stop
    $workingSet = [long]0
    $privateBytes = [long]0
    $cpu = [double]0
    $unavailable = 0
    foreach ($id in $ids) {
        try {
            $process = Get-Process -Id $id -ErrorAction Stop
            $workingSet += $process.WorkingSet64
            $privateBytes += $process.PrivateMemorySize64
            $cpu += $process.TotalProcessorTime.TotalSeconds
            $process.Dispose()
        }
        catch { $unavailable++ }
    }
    [pscustomobject]@{
        Timestamp = [DateTimeOffset]::Now.ToString("o")
        Mode = $Mode
        LauncherProcessId = $LauncherProcessId
        LauncherWorkingSetBytes = $root.WorkingSet64
        LauncherPrivateBytes = $root.PrivateMemorySize64
        DescendantCount = $ids.Count - 1
        TotalWorkingSetBytes = $workingSet
        TotalPrivateBytes = $privateBytes
        TotalCumulativeCpuSeconds = $cpu
        UnavailableProcesses = $unavailable
    }
    $root.Dispose()
    Start-Sleep -Seconds 1
}
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
$path = Join-Path $OutputDirectory "$Mode-$([DateTime]::Now.ToString('yyyyMMdd-HHmmss')).csv"
$samples | Export-Csv -NoTypeInformation -Path $path
Write-Output "Saved measurements: $path"
Write-Output "Descendants can include launched apps. Shared working-set pages may be counted more than once."
Write-Output "CPU values are cumulative; compare deltas only while the process set is unchanged."
