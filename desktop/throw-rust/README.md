# Throw Launcher — native Rust edition

An experimental Windows launcher rebuilt in Rust with **native Win32 controls**.
No WPF, .NET runtime, Electron, WebView2, browser process, or background telemetry.
The existing C# edition is preserved alongside this version.

## Run

Download `throw-rust-windows-x64` from a successful run of the
[Rust Windows workflow](https://github.com/maleksaadi0109/throw-launcher/actions/workflows/throw-rust-windows.yml).
Extract the ZIP into a new folder and run **Throw.Rust.exe**.
Windows may show a warning for this unsigned development executable.

- **Ctrl + Alt + Space** opens/hides the launcher (change it in Settings).
- Type an application or file name. Use Up/Down and Enter, or double-click.
- `file: budget` searches only files; ordinary queries show apps and files.
  Files need at least two characters and match a literal filename substring.
- `= (12 + 3) * 2` calculates; Enter copies the answer.
- `? search terms` opens DuckDuckGo in the default browser.
- An explicit `https://…` or `http://…` URL opens in the default browser.
- `>` shows Settings, Rebuild app index, and Quit commands.
- Escape and the close button hide the launcher. **Tray → Quit** exits it.
- The orange Throw icon in the tray also opens the Control Center.

The shortcut may conflict with the C# edition. Close it first, or choose a
different shortcut in either edition. A second Rust launch opens the existing
window rather than starting another background instance.

## Control Center

- Lite mode **on by default**: at most 5,000 indexed app shortcuts; Standard:
  20,000. Only Start menu and Desktop folders are scanned, once at startup or
  explicitly on request. There is no whole-drive index or background polling.
- Ink/orange and Paper/orange appearance.
- 4–12 displayed results; more than eight use the bounded scrollable list.
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

Settings and a bounded disk index live in `%APPDATA%\ThrowLauncherRust`.
For portable use, start `Throw.Rust.exe --portable`; its independent profile
is `ThrowRustData` next to the executable. Always use the same flag for this
profile. Neither edition reads/writes Flow's or the C# edition's profile.
`--background` suppresses the initial window (used by Windows startup).

## File search: Windows Search

File search uses **Windows Search's existing disk-backed SystemIndex**, accessed
directly through native ADO COM and `Search.CollatorDSO`. It does not start
PowerShell, load .NET, create a second full-drive catalog, or scan your drives.
The Windows Search service owns indexing and its separate resource usage.
The Rust app keeps only 24 file candidates per query; apps remain limited to
5,000/20,000 entries and the combined list remains limited to 4–12 results.
When files match, up to half the list is reserved for app matches; `file:` uses
the entire list for files. File rows show their full path below the filename.

Only files in Windows-indexed locations can appear. Adjust **Windows Settings →
Searching Windows / Indexing Options** to include folders; newly created, moved,
or deleted files may lag until Windows refreshes its catalog. Network/UNC and
device paths are not supported. There is no fallback drive crawl. Windows Search
disabled/unavailable, provider failures and unsupported wildcard characters
(`%`, `_`, `[` and `]`) produce a status message while app search keeps working.
The app does not change Windows indexing settings during normal use.

Queries are debounced by 150 ms and run on a single background worker with a
two-second provider connection/command timeout. Results from an old query are
discarded, rather than replacing current results. The provider controls actual
cancellation timing; if it stalls, app search and the window remain responsive
without creating more worker threads. Special calculator/web/command/URL queries
and empty/one-character queries do not contact the file provider.

Enter opens only a selected, validated, existing local file through its Windows
association, with no command-line arguments. Missing/inaccessible files and
association failures are reported. Files, including executable files and scripts,
can run their registered action when explicitly selected: open only files you
trust. Typed text is never passed to a shell or executed as a command.

## What is not ported

This is a working foundation, **not full Flow feature parity**. Existing .NET,
Python and JavaScript plugins cannot be loaded. Plugin marketplace, Everything
integration, UWP-only app discovery, arbitrary shell commands, custom themes,
extension APIs, rich icon previews, multilingual UI, automatic updating and a
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

The workflow also enables Windows Search on its disposable runner and runs
`--file-search-benchmark`. This opt-in mode creates 1,001 temporary files, adds
only their temporary folder to Windows Search, waits for indexing, checks
apostrophe/Unicode search, and measures 30 bounded mixed searches with each
5,000/20,000-app index. It removes its indexing rule and temporary files afterward.
Do not use this flag as a normal launch option: it deliberately modifies the
Windows indexing scope temporarily for measurement.
`CI-file-search-benchmark.txt` records first/median/p95/max query latency
(including COM overhead, excluding debounce) and working set/private committed
memory before/after each workload. Windows Search service memory is excluded,
so this is not a total system-cost or representative desktop idle benchmark.
The run fails rather than reporting invented timings when Search is unavailable
or fixtures do not index. Run this locally on Windows with
`./target/release/throw-rust.exe --file-search-benchmark`; the report is written
beside the executable. Errors are written to `smoke-error.txt` in the working
directory. Indexing/benchmark failures also have a separate CI evidence artifact.

Architecture: `config` validates/persists settings, `index` scans bounded app
folders, `search` ranks top-k results and constructs allowlisted actions, `files`
defines bounded file-query/merge policy,
`calculator` parses arithmetic without evaluation, and `windows` owns native UI
and OS calls. Win32 calls are kept out of the portable search engine.

## License and origin

MIT. This Rust implementation is newly written for Throw Launcher; it is not a
compiled conversion of Flow. The repository also retains its C# fork of
[Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) and the original
Flow/Wox license and credits. The existing Throw logo is reused.
