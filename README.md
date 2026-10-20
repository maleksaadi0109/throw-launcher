<p align="center">
  <img src="branding/throw-launcher.png" width="128" height="128" alt="Throw Launcher logo" />
</p>

<h1 align="center">Throw Launcher</h1>
<p align="center"><strong>A Windows launcher with more control over what stays in memory.</strong></p>
<p align="center">
  <a href="https://github.com/maleksaadi0109/throw-launcher/actions/workflows/flow-lite-windows.yml"><img alt="Windows build" src="https://github.com/maleksaadi0109/throw-launcher/actions/workflows/flow-lite-windows.yml/badge.svg" /></a>
  <img alt="Windows x64" src="https://img.shields.io/badge/platform-Windows_x64-122021" />
  <img alt="Experimental" src="https://img.shields.io/badge/status-experimental-FD662B" />
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-122021" /></a>
</p>

Throw Launcher is an independent, lightweight-focused development fork of
[Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher). It keeps the
original keyboard-first application and plugin architecture, adding reversible
resource controls rather than replacing the launcher with a web wrapper.

> **Development build, not a production release.** Windows runtime verification
> and before/after memory benchmarks are not complete. Smaller cache limits are
> implementation choices—not measured proof of a particular RAM saving.

## What is different?

| Feature | Behavior |
| --- | --- |
| **Reversible Lite mode** | Enabled by default in this fork. Turn it off and restart to restore the original resource policies without overwriting saved appearance or plugin preferences. |
| **Smaller icon cache** | Defaults to 50 entries in Lite mode instead of the original 150. Choose 25, 50, 100 or 150. Essential fallback icons remain available. |
| **Less startup image work** | Previously used icons are not preloaded in Lite mode unless you enable that option. Requested icons still load normally. |
| **First-use plugins** | Calculator and URL opt into delayed assembly loading and initialization. They load on a normal query or when their settings are expanded. A global plugin can load on the first general search. |
| **Reduced UI overhead** | Lite mode suppresses search-window animations and recycles result rows. Search result cloning retains its compatibility safeguards. |
| **Resource reports** | Refresh memory, process, heap, icon-cache and plugin statistics only when needed. Copy the report or export it as a text file—no monitoring service. |
| **Restart notices** | A notice appears when resource settings differ from the running profile. Reverting the changes clears it; the notice does not restart the app automatically. |

There are **no forced-GC timers, “RAM cleaner” tricks, or arbitrary plugin
suspension**. Plugins with background responsibilities remain on the original
startup path unless they explicitly declare compatibility.

## Try a Windows test build

There is no signed installer or stable Throw Launcher release yet.

If the [Windows workflow](https://github.com/maleksaadi0109/throw-launcher/actions/workflows/flow-lite-windows.yml)
has succeeded, its run contains a `throw-launcher-windows` artifact. Otherwise,
build the source:

### Requirements

- Windows x64.
- [.NET 9 SDK](https://dotnet.microsoft.com/download/dotnet/9.0).
- [PowerShell 7](https://github.com/PowerShell/PowerShell).

```powershell
git clone https://github.com/maleksaadi0109/throw-launcher.git
cd throw-launcher
pwsh ./desktop/Flow.Launcher/Scripts/build-lite.ps1
```

The script builds the solution and creates a self-contained portable test ZIP
under `generated-artifacts/windows/`. It does not replace an installed profile,
delete older test packages, or terminate a running launcher. Choose another
`-OutputDirectory` if a package folder already exists.

**Important:** test in the portable package, back up your existing settings, and
do not run the original launcher and this fork simultaneously. The internal
executable/assembly names remain `Flow.Launcher` for compatibility. Disable
official application auto-updates while evaluating this fork; an upstream
update could replace the modified application.

## Use Lite mode

Open **Settings → General → Lite mode**:

1. Choose the icon-cache limit and optional historical-image preload.
2. Choose whether compatible plugins can wait until first use.
3. Restart after changing these settings.

In a compatible plugin’s expanded settings, **Load at startup (restart)**
overrides first-use loading. Existing plugin enable/disable controls remain
available. A mode switch does not unload assemblies that are already running.

Open **General → Resource usage**, refresh a sample, then copy or export it.
Reports contain resource statistics, not full logs or settings files. Review a
report before sharing it publicly.

## Verification and measurements

Portable checks cover single initialization under concurrent searches,
cancellation isolation, failure retention, source syntax and modified XML.

```powershell
dotnet run --project verification/Flow.Launcher.LiteChecks -c Release
```

Focused Windows tests cover settings preservation, cache bounds, optional
preloading, serialization and restart-state detection. The GitHub workflow runs
them after building the Windows solution.

For comparable idle measurements on Windows:

```powershell
pwsh ./desktop/Flow.Launcher/Scripts/measure-lite.ps1 `
  -LauncherProcessId 1234 -Mode lite -Seconds 60
```

Replace `1234` with the launcher’s actual process ID. Repeat with `-Mode normal`
after restarting with the same plugins, index and theme. Process-tree totals
can include applications launched from the launcher and can count shared
working-set pages more than once.

See the [full measurement protocol](desktop/Flow.Launcher/LITE_MODE.md) before
claiming a memory improvement. Search latency, especially on first use, matters
alongside idle RAM.

## Repository layout

```text
branding/                         New Throw Launcher logo: SVG, PNG, ICO
desktop/Flow.Launcher/             Original desktop source and fork changes
verification/                     Portable lifecycle and source checks
.github/workflows/                Windows build and focused tests
generated-artifacts/              Ignored local packages and measurements
```

## Contributing

Small, measurable changes are welcome. Keep normal mode available, preserve
user preferences and avoid adding background work for optional features.
Plugin lazy loading is opt-in: do not assume every plugin can safely be delayed.

For a bug report, include Windows version, enabled plugins, normal/Lite mode,
reproduction steps and a reviewed resource report if relevant. Never include
passwords, access tokens or private settings.

## Credits and license

Based on **Flow Launcher**, originally downloaded from upstream revision
`3b89277c65981c8228f2c0525270a5a716506cca`.
Throw Launcher is an independent fork and is not an official Flow Launcher
release or an endorsement by its maintainers.

The original [MIT license](LICENSE), contributor credits and
[third-party attribution](desktop/Flow.Launcher/attribution.md) are preserved.
Upstream namespaces, plugin identifiers and much of the original documentation
are intentionally retained.

The new Throw Launcher mark is included in [branding](branding/README.md).
No new email account or contact address is implied; use this repository’s
[issues](https://github.com/maleksaadi0109/throw-launcher/issues).
