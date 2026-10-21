# Throw Launcher — native Rust edition

An experimental Windows launcher rebuilt in Rust with **native Win32 controls**.
No WPF, .NET runtime, Electron, WebView2, browser process, or background telemetry.
The existing C# edition is preserved alongside this version.

## Run

Download `throw-rust-windows-x64` from a successful run of the
[Rust Windows workflow](https://github.com/maleksaadi0109/throw-launcher/actions/workflows/throw-rust-windows.yml).
Extract the ZIP into a new folder and run **Throw.Rust.exe**.
Windows may show a warning for this unsigned development executable.

### Native command palette

Type **`>`** to browse commands, or search by name (`reboot`, `shutdown`,
`lock`, `sleep`, `hibernate`, `task manager`, `display`, `wifi`, `powershell`).
Commands share the three-row scrollable results list with keyboard activation.
Basic Arabic aliases are included for common power actions; the UI remains English.

- Restart, shutdown, advanced recovery restart, sign out, sleep and hibernate
  require a confirmation with **No selected by default**. Save your work first.
  Windows is never instructed to force-close applications.
- Lock your session; open or empty the Recycle Bin. Emptying it is permanent,
  applies to all drives and requires a separate explicit confirmation.
- Open Windows Settings and 12 common settings pages, Control Panel, Task
  Manager and Windows indexing options. Hardware/Windows policies may limit
  settings pages, sleep or hibernation; failures are reported, not hidden.
- Open Command Prompt or PowerShell (without a profile). This opens a terminal,
  **not** an arbitrary typed command executor.
- Open Throw's profile, measure memory, switch Pearl/Graphite, rebuild the
  index, open Throw Settings or quit.

Query text is untrusted: bounded text only selects fixed enum actions. System
executables come from the Windows system directory with separate constant
arguments; Windows settings/Shell destinations come from a closed catalog.
Result text is rendered as native text, not HTML. Typed arbitrary protocols,
shell arguments and scripts are not accepted.

This is the first native core-functionality expansion, **not full Flow parity**.
Browser bookmarks, configurable web providers, advanced file search/preview,
drag-and-drop/Dialog Jump, search history, game mode, automatic updates and the
complete Settings redesign still require further work. Original Flow plugins
are explicitly deferred; no .NET/Python/Node plugin runtime is loaded.

- **Ctrl + Alt + Space** opens/hides the launcher (change it in Settings).
- Type an application name. Use Up/Down and Enter, or double-click.
- Search local **folders** by name too. Personal folders and a bounded set of
  fixed-drive directories are indexed in the background. Type an absolute path
  such as `D:\Projects\` to browse beyond that index.
- The launcher always displays **three rows**. Scroll the wheel/scrollbar or use
  the arrow keys to reach more results; Settings controls the total result limit
  (25, 50 or 100), not the window height. Existing 4–12-result settings migrate to 50.
- **Ctrl+A / C / X / V / Z** select all, copy, cut, paste and undo in the search
  field, including when keyboard focus is on the results.
- **Alt+1–9** opens the corresponding result.
- Right-click an app (or focus the results and press **Shift+F10**) for Open,
  Run as administrator, Run as different user, Open containing folder,
  Open target folder, Copy path, and Hide launcher. Windows handles permission
  prompts; some apps do not support alternate-user launch. Store-only links may
  not expose a filesystem target; an explanation is shown instead.
- `= (12 + 3) * 2` calculates; Enter copies the answer.
- `? search terms` opens DuckDuckGo in the default browser.
- An explicit `https://…` or `http://…` URL opens in the default browser.
- `>` shows Settings, Rebuild search index, and Quit commands.
- Escape hides the launcher. **Tray → Quit** exits it.
- Click the simplified Throw tray icon to open the launcher; use Settings to
  open the Control Center.

The shortcut may conflict with the C# edition. Close it first, or choose a
different shortcut in either edition. A second Rust launch opens the existing
window rather than starting another background instance.

## Control Center

- Lite mode **on by default**: at most 5,000 indexed app shortcuts; Standard:
  20,000. Only Start menu and Desktop folders are scanned, once at startup or
  explicitly on request. There is no whole-drive index or background polling.
- macOS-inspired Pearl and Graphite appearances with soft neutral surfaces and
  quiet blue selection. Fresh profiles default to Pearl; saved theme choices remain.
- Native app icons in search results, loaded by a background Shell worker.
  Only the current result set requests icons; the cache is bounded to 48 in
  Lite mode or 96 in Standard. Evicted icon handles are destroyed, and loading
  failures use the Throw fallback instead of blocking the search box.
- A compact, fixed three-row search window, numbered keyboard hints, and a
  minimal original Throw logo. Drag its top edge or footer to move it.
- Folder names/paths only: Lite keeps at most 10,000 folder entries, Standard
  30,000, with additional byte, traversal, depth and time budgets. Personal known
  folders are prioritized; fixed-drive scanning is shallow. OS/application caches,
  hidden/system directories and reparse-point recursion are excluded.
  This is **not an exhaustive whole-disk index**. Direct local path browsing
  covers other locations on request; automatic network-share discovery is excluded.
- Folder rows open in Windows Explorer and support Open, containing folder,
  Copy path and Hide in their right-click menu.
- Enable/disable calculator, web search, local launch history, startup hiding,
  and hide-on-focus-loss.
- Select Ctrl+Alt+Space, Alt+Space, or Ctrl+Shift+Space. Conflicts are reported.
- Optional launch at Windows sign-in, under its own **ThrowLauncherRust** entry.
- Clear history with confirmation. Frequency history is local, bounded to
  100 apps, and can be disabled (disabling clears it).
- Measure working set and private committed memory **only when requested**.
- Optional preference import copies only `HideOnStartup` and
  `HideWhenDeactivated` from a selected Flow/WPF JSON file after confirmation.
  The current Rust settings are backed up; the source is unchanged. No code,
  plugins, paths, proxy settings, or shortcuts are imported; Lite stays on.

Settings and bounded app/folder caches live in `%APPDATA%\ThrowLauncherRust`.
For portable use, start `Throw.Rust.exe --portable`; its independent profile
is `ThrowRustData` next to the executable. Always use the same flag for this
profile. Neither edition reads/writes Flow's or the C# edition's profile.
`--background` suppresses the initial window (used by Windows startup).

## What is not ported

This is a working foundation, **not full Flow feature parity**. Existing .NET,
Python and JavaScript plugins cannot be loaded. Plugin marketplace, Everything
file search, UWP-only app discovery, arbitrary shell commands, custom themes,
extension APIs, custom icon packs, multilingual UI, automatic updating and a
signed installer are not implemented. Shortcuts to packaged applications in the
Start menu can still work.

The app deliberately does not execute typed shell commands or arbitrary URL
protocols. Application shortcuts in your own Start menu/Desktop are executable
content: only install shortcuts you trust. HTTP(S) opens are user-initiated,
including local/intranet addresses, and are handed to the browser—not fetched
by the launcher. Web searches send entered terms to DuckDuckGo.

## Memory evidence

Rust and avoiding a web runtime reduce sources of overhead, but **50 MB is
not guaranteed**. Use Control Center → Measure memory now, then compare with
Windows Task Manager after the same index has finished loading. Compare both
editions with equivalent shortcuts and features.

`CI-memory-sample.txt` in the package is a single Windows CI smoke-test sample,
not an idle benchmark on your PC. Working set and private committed memory are
different metrics. The smoke test exercises native search/list controls,
calculator result binding, embedded icon, settings controls and profile writes.
It also verifies real Shell icons reach the result list and captures both
appearance modes. The screenshots are a separate workflow artifact.
It does not prove global-hotkey/tray interaction on every Windows desktop.

There are no forced-GC timers, working-set trimming tricks, periodic resource
monitors, preloaded application icons, plugin runtimes or animated backgrounds.
Release builds use LTO and size optimization.

## Build and test

Requires Windows x64, stable Rust, and Visual Studio C++ Build Tools with a
Windows SDK (the normal MSVC Rust requirements).

```powershell
cd desktop/throw-rust
cargo test --locked
cargo build --release --locked
./target/release/throw-rust.exe
```

Cross-platform engine tests can also run on Linux/macOS using `cargo test`.
The Windows workflow compiles the actual UI and runs `--smoke-test` before
packaging. This flag creates/deletes its own temporary profile; it never changes
normal settings, startup registration, clipboard, or launches another app.

Architecture: `config` validates/persists settings, `index` scans bounded app
folders, `search` ranks top-k results and constructs allowlisted actions,
`calculator` parses arithmetic without evaluation, and `windows` owns native UI
and OS calls. Win32 calls are kept out of the portable search engine.

## License and origin

MIT. This Rust implementation is newly written for Throw Launcher; it is not a
compiled conversion of Flow. The repository also retains its C# fork of
[Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) and the original
Flow/Wox license and credits. The existing Throw logo is reused.
