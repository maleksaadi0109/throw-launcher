# Throw Launcher — native Rust edition

## Compact desktop appearance

The launcher is 640 logical pixels wide with three visible results. It stays
out of the Windows taskbar and Alt+Tab; use the global shortcut or notification
area icon to reopen it. Right-click the tray icon to open Settings or quit.
The same blue launch mark is embedded in the executable and used by the tray
and Settings window. Windows may retain an older executable icon in its cache;
extract a new build into a fresh folder before starting it.

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
Advanced file search/preview,
drag-and-drop/Dialog Jump, search history, game mode, automatic updates and the
other Flow search capabilities still require further work. Original Flow plugins
are explicitly deferred; no .NET/Python/Node plugin runtime is loaded.

- **Ctrl + Alt + Space** opens/hides the launcher (change it in Settings).
- Type an application or file name. Use Up/Down and Enter, or double-click.
- `file: budget` searches only files; ordinary queries include apps, folders and files.
  File queries need at least two characters and match a literal filename substring.
- Search local **folders** by name too. Personal folders and a bounded set of
  fixed-drive directories are indexed in the background. Type an absolute path
  such as `D:\Projects\` to browse beyond that index.
- The launcher always displays **three rows**. Scroll the wheel/trackpad or use
  the arrow keys to reach more results; Settings controls the total result limit
  (25, 50 or 100), not the window height. Existing 4–12-result settings migrate to 50.
  The vertical scrollbar is hidden; scrolling and keyboard navigation still work.
- Normal `settings` searches show Windows settings, not Throw preferences.
  Open Throw preferences with the Settings button or explicit `> settings`.
- **Ctrl+A / C / X / V / Z** select all, copy, cut, paste and undo in the search
  field, including when keyboard focus is on the results.
- **Alt+1–9** opens the corresponding result.
- Right-click an app (or focus the results and press **Shift+F10**) for Open,
  Run as administrator, Run as different user, Open containing folder,
  Open target folder, Copy path, and Hide launcher. Windows handles permission
  prompts; some apps do not support alternate-user launch. Store-only links may
  not expose a filesystem target; an explanation is shown instead.
- `= (12 + 3) * 2` calculates; Enter copies the answer.
- `? search terms` uses the `?` web provider (DuckDuckGo by default).
  Add keyword providers such as `wiki search terms` in Control Center → Search →
  **Bookmarks & web providers**.
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
- Opt in to browser bookmark search and create, edit, enable/disable or delete
  up to 16 web providers with keyword shortcuts (details below).
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

## File search: Windows Search

File search uses **Windows Search's existing disk-backed SystemIndex**, accessed
directly through native ADO COM and `Search.CollatorDSO`. It does not start
PowerShell, load .NET, create a second full-drive file catalog, or crawl drives
for files. The separate bounded folder index described above remains available.
The Windows Search service owns file indexing and its separate resource usage.
The Rust app keeps only 24 file candidates per query. The combined app, folder,
command and file list respects the configured total limit (up to 100); the
visible window remains three rows. When files match, up to half the list is
reserved for existing app/folder/command matches. `file:` uses the list for files
only. File rows show their full path below the filename.

Only files in Windows-indexed locations can appear. Adjust **Windows Settings →
Searching Windows / Indexing Options** to include folders; newly created, moved,
or deleted files may lag until Windows refreshes its catalog. Network/UNC and
device paths are not supported. There is no fallback file crawl. Windows Search
disabled/unavailable, provider failures and unsupported wildcard characters
(`%`, `_`, `[` and `]`) produce a status message while app search keeps working.
The app does not change Windows indexing settings during normal use.

Queries are debounced by 150 ms and run on a single background worker with a
two-second provider connection/command timeout. Stale query results are discarded.
The provider controls actual cancellation timing; if it stalls, app search and
the window remain responsive without creating more worker threads. Calculator,
web, explicit command and URL queries and empty/one-character queries do not
contact the file provider.

Enter opens only a selected, validated, existing local file through its Windows
association, with no command-line arguments. Missing/inaccessible files and
association failures are reported. Files, including executable files and scripts,
can run their registered action when explicitly selected: open only files you
trust. Typed text is never passed to a shell or executed as a command.

## What is not ported

This is a working foundation, **not full Flow feature parity**. Existing .NET,
Python and JavaScript plugins cannot be loaded. Plugin marketplace, Everything
integration, UWP-only app discovery, arbitrary shell commands, custom themes,
extension APIs, custom icon packs, multilingual UI, automatic updating and a
signed installer are not implemented. Shortcuts to packaged applications in the
Start menu can still work.

The app deliberately does not execute typed shell commands or arbitrary URL
protocols. Application shortcuts in your own Start menu/Desktop are executable
content: only install shortcuts you trust. HTTP(S) opens are user-initiated,
including local/intranet addresses, and are handed to the browser—not fetched
by the launcher. Web searches send entered terms to the selected provider.

## Bookmarks and web providers

Open **Control Center → Search → Bookmarks & web providers**. These are actual native
settings controls; no manual JSON editing or original Flow plugins are needed.

### Browser bookmarks (opt-in)

- Off in fresh and migrated profiles, including Lite mode. Enabling the switch
  and clicking **Apply switches** starts a bounded background snapshot. Disabling
  it immediately removes bookmark results and clears their in-memory snapshot;
  background discovery checks permission before each file read, and a scan
  already in flight is not used while disabled.
- Supported on Windows: **Chrome, Microsoft Edge, Brave and Chromium** under
  their standard `%LOCALAPPDATA%` user-data directories, in **Default** and
  **Profile 1–8**. Only their Chromium JSON `Bookmarks` files are read. The browser
  need not be running. Nothing is written to browser profiles.
- Search by bookmark title or URL. Results are labeled **Bookmark**, identify
  their browser/profile, and Enter/double-click opens the validated HTTP(S) URL
  in the default browser, through the existing URL action. Duplicate URLs are
  shown once. Bookmark launches are not added to app frequency history.
- At most 16 existing files, 2 MiB per file, 8 MiB of input per snapshot, 4,000
  retained bookmarks, 12,000 nodes per file and 32 folder levels. Oversized or
  malformed files, unsafe entries and reached limits are reported in the editor
  and launcher status. Missing standard profiles are normal, not errors.
  Symlink/reparse-point paths are skipped; browser profiles are not a security
  boundary against other software modifying files concurrently.
- **Refresh bookmarks / index** or the existing rebuild command refreshes the
  snapshot. There is no watcher, polling, sync, browser process, favicon download,
  persistent bookmark cache or recursive search for browser profiles.
- Not supported: Firefox/SQLite, Safari, Opera, nonstandard/portable browser
  locations, profiles beyond Profile 8, cloud-only favorites, bookmark editing
  and importing browser extensions. These limitations do not imply full Flow
  functionality has been ported.

### Keyword web providers

1. Choose **New**, enter a name, keyword and HTTP(S) URL template containing
   exactly one **`{query}`**, then click **Save provider**.
   Example: `wiki` with `https://en.wikipedia.org/w/index.php?search={query}`.
2. Select a saved provider to edit it. **Provider enabled** is saved with that
   provider; **Delete provider** requires confirmation. The global web switch
   is saved with **Apply switches**. Each Save/Delete/Apply persists immediately;
   closing the editor discards unsaved field edits.
3. Type `wiki Rust language` in the launcher. Keywords are case-insensitive,
   unique, start with an ASCII letter and contain up to 16 ASCII letters/digits.
   `?` is the special default keyword, and `?terms` still works. It can be edited,
   disabled or deleted like any provider. Deleting all providers is supported.

Query substitution percent-encodes UTF-8 bytes, including `&`, `#`, `?` and spaces;
terms cannot inject extra URL parameters or commands. A placeholder in the host,
credentials, invalid ports, malformed escapes, backslashes, control characters,
non-HTTP(S) protocols and unknown provider fields are rejected. URLs are checked
again before opening. Use ASCII/punycode hosts (or bracketed IPv6), explicit
lowercase `http://` or `https://`, and no credentials. The web switch disables
keyword searches, not deliberate direct HTTP(S) URL opens or enabled bookmarks.

Provider names/templates/counts and the whole settings file are bounded.
Validation and safe temporary-file replacement happen before live settings
change; errors are shown and failed updates leave the previous saved preferences
intact. Settings remain exclusively in Throw's own profile. Browser snapshots
stay in memory; provider definitions, not search terms, are saved.


## Native Settings

The Control Center uses a persistent six-section sidebar rather than a single
long form. It is native Rust/Win32, not a browser surface:

- **General:** Windows sign-in, hidden startup and focus-loss behavior.
- **Appearance:** Graphite and Pearl themes for both the launcher and Settings.
- **Search:** calculator, web search and total scrollable result limit.
- **Shortcuts:** global shortcut selection and a guide to native keyboard use.
- **Privacy:** local launch frequency, clear history, profile access and safe
  preference import with a backup.
- **Resources:** Lite indexing bounds, on-demand measured memory and edition
  limitations. No plugin marketplace or nonfunctional toggles.

Tab/Shift+Tab moves between visible controls; arrow keys change sections while
the sidebar is focused. Ctrl+1–6 jumps to a section and Ctrl+S saves. Enter
activates the focused button or saves by default. Esc/Cancel closes Settings,
asking before discarding edits. Switching sections keeps the draft intact.
Save keeps the current section open and reports success; errors keep the draft
available for correction. A theme command immediately saves Appearance and
updates an open Settings window without overwriting edits in other sections.
Import replaces the saved supported preferences only after confirmation; it
asks before discarding a draft, backs up the Rust settings and leaves its source
untouched. Folder-selection controls belong to the separate search-folder work.

Settings uses per-monitor DPI awareness with scaled fonts, controls and sidebar
rows. Its awareness is separate from the existing system-aware launcher.

## Memory evidence

Rust and avoiding a web runtime reduce sources of overhead, but **50 MB is
not guaranteed**. Use Settings → Resources → Measure memory now, then compare with
Windows Task Manager after the same index has finished loading. Compare both
editions with equivalent shortcuts and features.

`CI-memory-sample.txt` in the package is a single Windows CI smoke-test sample,
not an idle benchmark on your PC. Working set and private committed memory are
different metrics. The smoke test exercises native search/list controls,
calculator result binding, embedded icon, settings controls and profile writes.
It also verifies real Shell icons reach the result list and captures both
appearance modes. Settings checks cover all six sections in both themes,
Tab/Shift+Tab and sidebar arrows, Ctrl+number/Ctrl+S, draft preservation,
save/reload, write-failure recovery, safe import and command-to-theme
synchronization. Synthetic 96/144/192-DPI messages verify the per-monitor layout
handler; these do not replace dragging between real mixed-DPI monitors.
The screenshots are a separate workflow artifact.
Bookmark/provider smoke coverage uses temporary Chromium fixtures, binds real
native search results, exercises provider create/edit/disable/delete and global
switches, rejects invalid/failed saves, and checks profile reload. It never reads
personal browser profiles or launches a browser. Compiling this smoke code on
Linux does **not** constitute a Windows runtime pass; that evidence comes from
the Windows workflow.
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

### Measured Windows sample

[Successful Windows run, 2026-10-21](https://github.com/maleksaadi0109/throw-launcher/actions/runs/39594725804)
built and packaged the file-search release, passed all 16 tests then present and
native smoke checks (including file-result binding and deleted-file rejection),
and ran the real Windows Search fixture on a `windows-latest` x64 runner.
This is evidence for the file-search implementation before integration with the
newer three-row/folder/icon UI, not a measurement of the combined edition:

| App entries | First query | Median | p95 | Maximum | Working set after queries | Private committed |
| --- | --- | --- | --- | --- | --- | --- |
| 5,000 | 23.01 ms | 22.34 ms | 27.46 ms | 27.77 ms | 19.5 MB | 4.2 MB |
| 20,000 | 25.51 ms | 26.24 ms | 31.20 ms | 60.13 ms | 23.0 MB | 7.8 MB |

Before loading the synthetic app indexes, the search-enabled process used
18.3 MB working set / 3.2 MB private committed. After dropping them it used
18.6 MB / 3.3 MB. These are process snapshots, **not peak-memory guarantees**.
The file catalog contained 1,001 test files, not a representative full desktop
library; Windows Search service costs and the 150 ms UI debounce are excluded.
The first measured query follows fixture-readiness queries, so it is **not a
cold-cache measurement**. See the run's `throw-rust-windows-x64` package for the
unabridged `CI-file-search-benchmark.txt`.

Architecture: `config` validates/persists settings, `index` scans bounded app
folders, `search` ranks top-k results and constructs allowlisted actions, `files`
defines bounded file-query/merge policy,
`calculator` parses arithmetic without evaluation, and `windows` owns native UI
and OS calls. `bookmarks` reads bounded Chromium snapshots; `web` validates
provider schemas, encodes queries and allowlists HTTP(S) destinations. Win32 calls
are kept out of the portable search engine.

## License and origin

MIT. This Rust implementation is newly written for Throw Launcher; it is not a
compiled conversion of Flow. The repository also retains its C# fork of
[Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher) and the original
Flow/Wox license and credits. The existing Throw logo is reused.
