<p align="center"><img src="branding/throw-launcher.png" width="112" alt="Throw Launcher logo"></p>
<h1 align="center">Throw Launcher</h1>
<p align="center">Your apps. Your files. One shortcut.</p>
<p align="center">
  <a href="https://github.com/maleksaadi0109/throw-launcher/actions/workflows/throw-rust-windows.yml"><img alt="Windows build" src="https://github.com/maleksaadi0109/throw-launcher/actions/workflows/throw-rust-windows.yml/badge.svg"></a>
  <img alt="Windows x64" src="https://img.shields.io/badge/Windows-x64-1234C5">
  <img alt="Native Rust" src="https://img.shields.io/badge/Rust-native-24262C">
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-24262C"></a>
</p>

**Throw is a lightweight-focused Windows launcher built in Rust with native
Windows controls.** Search apps, folders and indexed files, perform quick
calculations, search the web, and run Windows commands without opening a large
dashboard. No Electron, WebView, .NET runtime, or background telemetry process.

> **Preview software:** the native edition is actively developed. It does not
> yet support Flow Launcher plugins, and it does not promise a fixed 50 MB RAM
> footprint. The installer is currently **unsigned**, not a signed stable release.

## Install Throw

1. Open [Windows builds](https://github.com/maleksaadi0109/throw-launcher/actions/workflows/throw-rust-windows.yml)
   and choose a **successful** run from `main`.
2. Download **throw-launcher-installer-x64** from its Artifacts section
   (GitHub may require you to sign in).
3. Extract the download and run **Throw-Launcher-Setup-x64.exe**.
4. Choose your installation folder and whether Throw should start when you
   sign in to Windows. Desktop shortcuts are optional.
5. Choose your appearance, keyboard shortcut, Lite mode, focus behavior, and
   local launch-history preference. Finish setup and open Throw.

The default location is `%LOCALAPPDATA%\Programs\Throw Launcher`.
Installation is for your Windows account and does not require administrator
rights or download an additional runtime.

**Already running an older copy?** Use **tray icon → Quit** first. Closing the
launcher window only hides it. The installer asks you to close a running copy
before replacing its files.

**Prefer portable?** Download **throw-rust-windows-x64**, extract
`Throw-Rust-Windows-x64.zip`, and run `Throw.Rust.exe`. Add `--portable` to keep
preferences in `ThrowRustData` beside the executable.

Windows may display a reputation or publisher warning for unsigned preview
builds. Only run a build from a source you trust; this project does not claim a
verified publisher signature.

## What you can do

| Feature | How it works |
| --- | --- |
| **Apps and folders** | Start typing a name. Use arrows, Enter, or Alt+1–9. |
| **File search** | Type `file: invoice`. Uses the existing Windows Search catalog rather than a second whole-disk index. Windows Search must be available. |
| **Quick calculations** | Type `= (12 + 3) * 2`, then Enter to copy the answer. |
| **Web and bookmarks** | Type `? search terms` or a full URL. Browser bookmark discovery is opt-in; keyword web providers are configurable. |
| **System commands** | Type `>` or search for Restart, Shut down, Lock, and other commands. Disruptive actions ask for confirmation. |
| **Inline actions** | Right-click a result or press **Ctrl+K** in search. Filter actions, use Alt+numbers, then Esc/Back to return to your search. |
| **File and web actions** | Open files, open their containing folder, copy paths, open URLs, or copy links. App-only elevation options are not shown for these results. |
| **Native editing** | Ctrl+A, C, X, V and Z work in the search field. |
| **Compact layout** | Three visible search rows, with scrolling for more; action panels show up to five rows. |
| **Your appearance** | Pearl light and Graphite dark themes, antialiased command icons, and a multi-resolution Throw logo. |

The default global shortcut is **Ctrl+Alt+Space**. Choose **Alt+Space** or
**Ctrl+Shift+Space** during installation or in Settings. A shortcut can be
unavailable if another app has already registered it; Throw reports that conflict.

## Designed to stay small

- Lite mode is enabled by default.
- Application icons load on demand through a bounded background cache.
- Original command icons use bounded, temporary supersampling for smooth edges
  rather than loading a large image library.
- Windows supplies application icons; older applications may only provide
  low-resolution artwork.
- File results reuse Windows Search. Its service cost is separate from Throw's
  process memory and must be included when comparing total system resource use.
- No forced garbage-collection loops or “RAM cleaner” tricks.

CI packages include a memory sample and file-search evidence from a disposable
Windows runner. These are measurements of that environment, **not a guarantee**
for your PC, app collection, or search index.

## Settings and upgrades

Open **Settings** from the launcher or tray menu. Preferences live in
`%APPDATA%\ThrowLauncherRust`.

Upgrades keep existing preferences unless you explicitly select **Apply the
preferences below** in setup. Startup and shortcut choices remain visible in
the installer. Uninstall through Windows **Installed apps → Throw Launcher**.
Uninstallation removes the program, its shortcuts, and its own startup entry,
but keeps your personal profile. It never removes Flow Launcher's profile.

## Build from source

On Windows with the Rust toolchain:

```powershell
cd desktop/throw-rust
cargo test --locked
cargo build --release --locked
.\target\release\throw-rust.exe
```

See the [native edition guide](desktop/throw-rust/README.md) for development,
keyboard controls, limitations, and verification details.
The [Windows workflow](.github/workflows/throw-rust-windows.yml) also builds the
Inno Setup installer and checks its real wizard, saved choices, upgrades, and
uninstallation on a disposable runner.

## Editions and credits

- **`desktop/throw-rust`** — the current native Rust preview described above.
- **`desktop/Flow.Launcher`** — the preserved C#/WPF development fork. Its engine
  and plugin contracts are separate from the Rust rebuild.

Throw began as a fork of [Flow Launcher](https://github.com/Flow-Launcher/Flow.Launcher).
Upstream licenses and notices are retained; original contributors deserve credit.
The native edition uses its own executable, profile, logo, and command artwork.
It is not affiliated with or endorsed by Flow Launcher, Apple, or Microsoft.

Licensed under [MIT](LICENSE). See the repository's preserved notices for
third-party components.
