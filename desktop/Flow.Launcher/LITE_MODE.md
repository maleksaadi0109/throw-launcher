# Throw Launcher — development fork

This is the original Windows/.NET/WPF launcher with reversible resource-saving
controls. It is not a web app or a rewrite. The upstream MIT license and
attribution remain included.

## Windows identity and the 50 MB target

The Windows executable title/product metadata is **Throw Launcher**, and its
embedded icon uses the project's existing orange Throw mark. The internal
filename is `Throw.Launcher.exe`, with a separate profile and singleton. A source update cannot
change an already-running binary: close the old launcher and start a freshly
built portable package. The build script checks the published executable's
FileDescription and ProductName before producing a ZIP, as well as checking
the icon source files.

**50 MB is a target, not an enforced limit or a measured result.** Task Manager
includes more than the managed heap: WPF, native libraries, images and loaded
plugins contribute. A screenshot showing 153.6 MB does not identify their
individual contributions. A 50 MB managed-heap limit would not cap total memory
and could instead cause out-of-memory failures.

Portable builds now default to a **Compact runtime profile**:

- Workstation GC instead of server GC.
- `System.GC.ConserveMemory = 9`, asking .NET to conserve managed heap space.
  More frequent collection can increase CPU use or search latency.
- The image-deduplication hash/path lookup is bounded to twice the selected icon
  cache capacity instead of retaining a history of every unique image.

This runtime profile is applied at process startup, independently of the Lite
mode switch. For a fair comparison, build both profiles into separate folders:

```powershell
pwsh ./desktop/Flow.Launcher/Scripts/build-lite.ps1 -MemoryProfile Compact -OutputDirectory ./generated-artifacts/compact
pwsh ./desktop/Flow.Launcher/Scripts/build-lite.ps1 -MemoryProfile Standard -OutputDirectory ./generated-artifacts/standard
```

Use identical Lite settings, plugins, workloads and machine conditions. Compare
idle memory, first-search latency and repeated-search latency. `BUILD-INFO.txt`
identifies the packaged runtime profile. No periodic forced collection,
working-set trimming or silent disabling of search features is used.

## Use

Open **Control Center → Workspace → Lite mode**. Lite mode is enabled by default in this
fork. Turn it off and restart for the original startup/cache/row behavior.
Existing animation, plugin-enabled, keyword, theme and search-delay preferences
are not rewritten.

- Lite icon cache defaults to 50 entries (normal mode: 150). Supported UI choices:
  25, 50, 100 and 150. Configuration values are bounded to 16–150.
- Previously used icons are not preloaded by default in Lite mode. Icons still
  load when requested. Essential fallback icons are retained separately.
- Search-window animations are suppressed in Lite mode; the saved normal-mode
  animation preference is retained.
- Result rows use recycling in Lite mode. Other mode uses the upstream policy.
- Empty/canceled result batches skip cloning, and nonempty clone buffers are
  pre-sized. Defensive copying is retained for plugin compatibility.
- Calculator and URL explicitly opt into first-use loading. Their assemblies
  and initialization wait until a normal query or expansion of their settings.
  A global `*` plugin will load on the first general search, not necessarily its
  first matching result. This primarily saves idle/startup resources.
- Other plugins remain on the original startup path. No arbitrary third-party
  runtime is suspended, unloaded, or force-killed.
- Essentials' **Resource details → Refresh snapshot** measures on request only.
  It reports launcher memory, descendant totals, heap, cache occupancy and
  initialized/deferred plugin counts. It does not claim precise memory
  attribution to individual in-process plugins.
- Copy or export a collected report as text. Export uses a user-selected
  destination with overwrite confirmation and excludes full logs/settings.
- A restart notice compares requested resource settings with the active
  startup profile, including per-plugin startup overrides. Reverting the
  changes clears it; the application is never restarted automatically.
- Existing per-plugin startup and enable/disable preferences are retained.
  Plugin-management pages are no longer exposed in the simplified Settings.

Restart after changing resource or plugin load controls. A running session may
already have loaded plugins; toggling Lite mode does not unload .NET assemblies.
There are no RAM-cleaner timers or forced collections.

## Throw interface and branding

The native search window now has a Throw identity bar and a direct Settings
button. Fresh profiles start with **Throw**, a rounded dark teal/orange theme;
existing profiles keep their selected theme. Select **Settings → Themes → Throw**
to apply it to an existing profile. Other themes and the theme editor remain.
The identity bar is present regardless of which search theme is selected.

Settings now contains only **Essentials, Themes, Shortcuts and About**. Essentials
keeps Lite mode, start with Windows, start hidden, hide on focus loss, language,
and an optional on-demand resource report. Shortcuts keeps the launcher hotkey
and result-preview hotkey. The new headings are English; the language selector
continues to affect supported existing interface text and search features.

Plugins, Plugin Store and HTTP Proxy are removed from the navigation, not from
the engine. Existing plugin configuration, proxy settings, custom shortcuts and
other advanced preferences are not erased or reset. App/file/calculator/URL
search still uses the original plugin architecture. About retains local settings
folder access and upstream MIT attribution, without upstream application-update
buttons that could replace this fork.

The executable's ApplicationIcon, window icon and tray release icon use the
existing Throw T/arrow artwork. The Windows packaging script refuses mismatched
icon sources and also includes `Throw-Launcher.ico` for shortcuts. The internal
executable is `Throw.Launcher.exe`; its filename does not
determine its icon. Copying an older executable does not incorporate these
changes: build the updated source first. Explorer may cache an earlier icon at
the same path, so inspect a newly extracted build in a fresh directory.

Verification here is limited to portable lifecycle/source/XML/resource-reference
and icon-source checks. These UI changes have **not** been rendered or exercised
on Windows, and no updated Windows executable or measured performance results
have been produced here.

## Build a Windows test package

On Windows with **PowerShell 7 and .NET 9 SDK**:

```powershell
./Scripts/build-lite.ps1
```

The script builds the solution and publishes a self-contained Windows x64
package under `generated-artifacts/windows` at the workspace root. It creates a
portable `ThrowData` directory so it does not replace an installed profile.
For an existing output folder, choose a new `-OutputDirectory`.
The repository-root GitHub workflow offers the same build and focused tests
when this project is pushed to a repository; it has not been run automatically.
Disable upstream automatic application updates while evaluating this fork:
an official update could replace the modified binaries. This is an unsigned
Windows x64 test package; it is not a production release.

## Measurement protocol — no results claimed yet

Portable verification completed here: eight first-use lifecycle checks passed,
including concurrent initialization, cancellation isolation and failure caching.
Roslyn parsed all 394 application C# source files without syntax errors; modified
project/XAML XML and the opt-in manifests were also validated. Focused Windows
settings/cache tests are included but have not been run here. Syntax validation
does not substitute for semantic compilation or rendering the application.

Windows baseline, visual verification, runtime tests and performance comparison
are **pending**. This Linux environment cannot launch WPF. A zero exit status
from a cross-targeting invocation is not sufficient proof of a complete build;
check that the executable and build summary were produced.

Use separate portable copies for upstream, this fork in normal mode, and this
fork in Lite mode, with identical plugins, application indexes and themes.
Do not run them simultaneously because the launcher is single-instance.
Back up profiles before copying test data. Never test against the only copy of
your installed profile.

1. Record Windows version, RAM, CPU, launcher revision and enabled plugins.
2. Measure cold and warm startup from logs, both through plugin readiness.
3. Wait 60 seconds, then sample idle for 60 seconds:

   ```powershell
   ./Scripts/measure-lite.ps1 -LauncherProcessId 1234 -Mode lite
   ```

   Substitute the actual PID. Repeat with `-Mode normal` after restarting.
4. Run the same app, calculator, URL, keyword and file searches. Record first-use
   latency separately from later searches, including p50 and p95 where possible.
5. Repeat open/search/close cycles for at least 15 minutes. Inspect retained
   memory after idle without forcing GC.
6. Test rapid typing/cancellation, large result scrolling, mouse selection,
   keyboard selection, hotkeys, context menus, icons, high DPI, sleep/wake,
   first-use settings access, disabled/re-enabled plugins, first-use failures
   and both directions of mode switching followed by restart.
7. In a profiler, inspect search-result allocations before replacing any
   plugin-facing result structure or shortening result lists.

Only accept changes that reduce measured resource usage without material
regression in search responsiveness. Do not claim a fixed MB or percentage
reduction until measurements exist. Process-tree totals can include applications
opened from the launcher; do not launch unrelated apps during idle comparisons.

## Compatibility contract for plugin developers

`SupportsLazyInitialization: true` in a plugin manifest is an explicit opt-in,
not automatic compatibility detection. Use it only when the plugin does not
need startup events, home results, dialog-jump registration or previews before
its first normal query/settings access. First-use initialization is shared by
concurrent callers. Canceling a query cancels its wait, not initialization for
other callers. Failed initialization is logged and reported; restart retries.
Unused plugins are not initialized just to shut them down.

The initial compatible set is intentionally small. Wider plugin adoption,
runtime idle shutdown, visual-effect changes beyond animations, manual-index
policies and any new shortcut features require separate compatibility work.
Existing shortcuts, index caches/watchers and cancellation mechanisms are
retained, not duplicated.
