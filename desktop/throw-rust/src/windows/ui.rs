use super::icons::{IconCache, ICONS_READY};
use super::platform::*;
mod snapshot;
mod visuals;
use std::{
    cell::{Cell, RefCell},
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::mpsc::{self, Receiver},
};
use throw_launcher::{
    config::{import_preferences, read_bounded, Hotkey, Settings, Theme},
    index::{self, Entry, Scan, LITE_INDEX_LIMIT, STANDARD_INDEX_LIMIT},
    search::{self, Action, ResultRow},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::*, SystemServices::SS_ENDELLIPSIS, Threading::*},
    UI::{Controls::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
};

const MAIN_CLASS: &str = "ThrowRustLauncherWindow";
const SETTINGS_CLASS: &str = "ThrowRustSettingsWindow";
const HOTKEY_ID: i32 = 41;
const TRAY: u32 = WM_APP + 1;
const INDEX_READY: u32 = WM_APP + 2;
const SEARCH: usize = 101;
const RESULTS: usize = 102;
const SETTINGS_BUTTON: usize = 103;
const OPEN: usize = 201;
const REINDEX: usize = 202;
const SETTINGS_MENU: usize = 203;
const QUIT: usize = 204;
const SAVE: usize = 301;
const IMPORT: usize = 302;
const CLEAR: usize = 303;
const MEMORY: usize = 304;
const FOLDER: usize = 305;
const LITE: usize = 310;
const HIDE_START: usize = 311;
const HIDE_FOCUS: usize = 312;
const CALCULATOR: usize = 313;
const WEB: usize = 314;
const HISTORY: usize = 315;
const STARTUP: usize = 316;
const HOTKEY: usize = 320;
const THEME: usize = 321;
const LIMIT: usize = 322;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static MAIN_WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
}

struct App {
    hwnd: HWND,
    settings_hwnd: HWND,
    edit: HWND,
    list: HWND,
    status: HWND,
    memory_label: HWND,
    instance: HINSTANCE,
    font: HFONT,
    small_font: HFONT,
    heading_font: HFONT,
    input_font: HFONT,
    background: HBRUSH,
    field: HBRUSH,
    selected: HBRUSH,
    icon: HICON,
    icons: Option<IconCache>,
    scale: f64,
    settings: Settings,
    root: PathBuf,
    entries: Vec<Entry>,
    rows: Vec<ResultRow>,
    worker: Option<Receiver<Result<Scan, String>>>,
    reindex_again: bool,
    portable: bool,
    popup: bool,
}

impl App {
    fn px(&self, value: i32) -> i32 {
        (value as f64 * self.scale).round() as i32
    }
    fn limit(&self) -> usize {
        if self.settings.lite {
            LITE_INDEX_LIMIT
        } else {
            STANDARD_INDEX_LIMIT
        }
    }
    fn text_color(&self) -> u32 {
        if self.settings.theme == Theme::Ink {
            rgb(235, 242, 239)
        } else {
            rgb(20, 38, 32)
        }
    }
    fn muted(&self) -> u32 {
        if self.settings.theme == Theme::Ink {
            rgb(155, 176, 165)
        } else {
            rgb(75, 95, 85)
        }
    }

    unsafe fn rebuild_brushes(&mut self) {
        for brush in [self.background, self.field, self.selected] {
            if !brush.is_null() {
                DeleteObject(brush);
            }
        }
        let ink = self.settings.theme == Theme::Ink;
        self.background = CreateSolidBrush(if ink {
            rgb(18, 32, 29)
        } else {
            rgb(242, 246, 243)
        });
        self.field = CreateSolidBrush(if ink {
            rgb(28, 46, 40)
        } else {
            rgb(255, 255, 255)
        });
        self.selected = CreateSolidBrush(if ink {
            rgb(52, 71, 55)
        } else {
            rgb(247, 224, 201)
        });
    }

    unsafe fn control(
        &self,
        parent: HWND,
        class: &str,
        text: &str,
        id: usize,
        style: u32,
        rect: [i32; 4],
    ) -> HWND {
        let hwnd = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            self.px(rect[0]),
            self.px(rect[1]),
            self.px(rect[2]),
            self.px(rect[3]),
            parent,
            id as HMENU,
            self.instance,
            null(),
        );
        SendMessageW(hwnd, WM_SETFONT, self.font as usize, 1);
        hwnd
    }

    unsafe fn create_controls(&mut self) -> Result<(), String> {
        self.edit = self.control(
            self.hwnd,
            "EDIT",
            "",
            SEARCH,
            WS_TABSTOP | ES_AUTOHSCROLL as u32,
            [70, 101, 580, 30],
        );
        SendMessageW(self.edit, EM_SETLIMITTEXT, 512, 0);
        SendMessageW(self.edit, WM_SETFONT, self.input_font as usize, 1);
        SendMessageW(
            self.edit,
            EM_SETCUEBANNER,
            0,
            wide("Search apps, = calculate, ? search web").as_ptr() as isize,
        );
        // Keyboard navigation belongs to the launcher, not to the edit control.
        if SetWindowSubclass(self.edit, Some(edit_proc), 1, 0) == 0 {
            return Err("Cannot attach search keyboard controls.".into());
        }
        self.list = self.control(
            self.hwnd,
            "LISTBOX",
            "",
            RESULTS,
            WS_TABSTOP
                | WS_VSCROLL
                | LBS_NOTIFY as u32
                | LBS_HASSTRINGS as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            [20, 176, 704, 448],
        );
        if SetWindowSubclass(self.list, Some(edit_proc), 2, 0) == 0 {
            return Err("Cannot attach result keyboard controls.".into());
        }
        SendMessageW(self.list, LB_SETITEMHEIGHT, 0, self.px(56) as isize);
        self.control(
            self.hwnd,
            "BUTTON",
            "Settings",
            SETTINGS_BUTTON,
            WS_TABSTOP | BS_OWNERDRAW as u32,
            [620, 26, 104, 34],
        );
        self.status = self.control(
            self.hwnd,
            "STATIC",
            "Preparing app index...",
            110,
            SS_ENDELLIPSIS as u32,
            [20, 638, 400, 24],
        );
        SendMessageW(self.status, WM_SETFONT, self.small_font as usize, 1);
        if [self.edit, self.list, self.status]
            .iter()
            .any(|h| h.is_null())
        {
            return Err("Windows could not create launcher controls.".into());
        }
        self.layout();
        self.refresh();
        Ok(())
    }

    unsafe fn layout(&self) {
        let mut client: RECT = zeroed();
        GetClientRect(self.hwnd, &mut client);
        let width = client.right;
        let height = client.bottom;
        MoveWindow(
            self.edit,
            self.px(68),
            self.px(103),
            (width - self.px(100)).max(1),
            self.px(29),
            1,
        );
        MoveWindow(
            GetDlgItem(self.hwnd, SETTINGS_BUTTON as i32),
            width - self.px(124),
            self.px(26),
            self.px(104),
            self.px(34),
            1,
        );
        MoveWindow(
            self.list,
            self.px(20),
            self.px(176),
            (width - self.px(40)).max(1),
            (height - self.px(220)).max(self.px(56)),
            1,
        );
        MoveWindow(
            self.status,
            self.px(20),
            height - self.px(30),
            if width >= self.px(520) {
                width - self.px(290)
            } else {
                width - self.px(40)
            },
            self.px(22),
            1,
        );
    }

    fn request_result_icons(&mut self) {
        if let Some(icons) = self.icons.as_mut() {
            icons.set_lite(self.settings.lite);
            icons.drain();
            icons.request(self.rows.iter().filter_map(|row| {
                match row.action {
                    Action::App(i) => self
                        .entries
                        .get(i)
                        .filter(|e| e.valid())
                        .map(|e| e.path.clone()),
                    _ => None,
                }
            }));
        }
    }

    unsafe fn set_status(&self, message: &str) {
        SetWindowTextW(self.status, wide(message).as_ptr());
    }

    unsafe fn refresh(&mut self) {
        let mut text = [0u16; 513];
        let count = GetWindowTextW(self.edit, text.as_mut_ptr(), text.len() as i32);
        let text = String::from_utf16_lossy(&text[..count.max(0) as usize]);
        self.rows = search::query(&text, &self.entries, &self.settings);
        self.request_result_icons();
        SendMessageW(self.list, LB_RESETCONTENT, 0, 0);
        for row in &self.rows {
            SendMessageW(
                self.list,
                LB_ADDSTRING,
                0,
                wide(&row.title).as_ptr() as isize,
            );
        }
        if !self.rows.is_empty() {
            SendMessageW(self.list, LB_SETCURSEL, 0, 0);
        }
        if self.worker.is_none() {
            self.set_status(&if self.rows.is_empty() && !text.trim().is_empty() {
                "No matches. Try ? web search, = calculator, or > commands.".into()
            } else {
                format!(
                    "{} apps  /  {} mode",
                    self.entries.len(),
                    if self.settings.lite {
                        "Lite"
                    } else {
                        "Standard"
                    }
                )
            });
        }
        ShowWindow(
            self.list,
            if self.rows.is_empty() {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        InvalidateRect(self.list, null(), 0);
        InvalidateRect(self.hwnd, null(), 0);
    }

    unsafe fn show(&self) {
        ShowWindow(self.hwnd, SW_SHOWNORMAL);
        SetForegroundWindow(self.hwnd);
        SetFocus(self.edit);
        SendMessageW(self.edit, EM_SETSEL, 0, -1);
    }

    unsafe fn reindex(&mut self) {
        if self.worker.is_some() {
            self.reindex_again = true;
            return;
        }
        let roots = match app_roots() {
            Ok(roots) => roots,
            Err(error) => {
                self.set_status(&error);
                return;
            }
        };
        self.set_status("Refreshing app index in the background...");
        let (sender, receiver) = mpsc::channel();
        let limit = self.limit();
        let cache = self.root.join("apps.json");
        let hwnd = self.hwnd as usize;
        self.worker = Some(receiver);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let scan = index::scan(&roots, limit);
                save_index(&cache, &scan.entries)?;
                Ok::<_, String>(scan)
            })
            .unwrap_or_else(|_| Err("App indexing failed unexpectedly.".into()));
            let _ = sender.send(result);
            // No timer and no polling. Wake the UI once the bounded scan finishes.
            unsafe {
                PostMessageW(hwnd as HWND, INDEX_READY, 0, 0);
            }
        });
    }

    unsafe fn index_ready(&mut self) {
        let result = self
            .worker
            .take()
            .and_then(|receiver| receiver.try_recv().ok());
        match result {
            Some(Ok(mut scan)) => {
                if let Some(icons) = self.icons.as_mut() {
                    icons.clear();
                }
                // Settings may have changed while the background scan ran.
                scan.entries.truncate(self.limit());
                self.entries = scan.entries;
                self.refresh();
                if scan.unreadable > 0 || scan.truncated {
                    self.set_status(&format!(
                        "{} apps • {} unreadable folders/items{}",
                        self.entries.len(),
                        scan.unreadable,
                        if scan.truncated {
                            " • index limit reached"
                        } else {
                            ""
                        }
                    ));
                }
            }
            Some(Err(error)) => {
                self.set_status(&error);
            }
            None => {
                self.set_status(
                    "App index worker did not return a result. Try > Rebuild app index.",
                );
            }
        }
        if self.reindex_again {
            self.reindex_again = false;
            self.reindex();
        }
    }

    unsafe fn activate(&mut self) {
        let selected = SendMessageW(self.list, LB_GETCURSEL, 0, 0);
        if selected < 0 {
            return;
        }
        let Some(row) = self.rows.get(selected as usize).cloned() else {
            return;
        };
        let result = match row.action {
            Action::App(i) => {
                if let Some(entry) = self.entries.get(i) {
                    if !entry.valid() || !entry.path.is_file() {
                        Err("The shortcut no longer exists. Rebuild the app index.".into())
                    } else {
                        let result = open(self.hwnd, entry.path.as_os_str());
                        if result.is_ok() {
                            self.settings
                                .record_launch(entry.path.to_string_lossy().into_owned());
                            if let Err(error) = self.settings.save(&self.root.join("settings.json"))
                            {
                                message(self.hwnd, &error, true);
                            }
                        }
                        result
                    }
                } else {
                    Err("App index changed. Search again.".into())
                }
            }
            Action::Url(url) => {
                if search::safe_url(&url).is_none() {
                    Err("URL is not allowed.".into())
                } else {
                    open(self.hwnd, std::ffi::OsStr::new(&url))
                }
            }
            Action::Copy(value) => copy_text(self.hwnd, &value),
            Action::Settings => {
                self.show_settings();
                return;
            }
            Action::Reindex => {
                self.reindex();
                return;
            }
            Action::Quit => {
                DestroyWindow(self.hwnd);
                return;
            }
        };
        match result {
            Ok(()) => {
                ShowWindow(self.hwnd, SW_HIDE);
            }
            Err(error) => message(self.hwnd, &error, true),
        }
    }

    unsafe fn show_settings(&mut self) {
        if !self.settings_hwnd.is_null() {
            ShowWindow(self.settings_hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(self.settings_hwnd);
            return;
        }
        self.settings_hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(SETTINGS_CLASS).as_ptr(),
            wide("Throw • Control Center").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            self.px(660),
            self.px(710),
            self.hwnd,
            null_mut(),
            self.instance,
            null(),
        );
        let parent = self.settings_hwnd;
        if parent.is_null() {
            message(self.hwnd, "Cannot open Control Center.", true);
            return;
        }
        SendMessageW(parent, WM_SETICON, ICON_BIG as usize, self.icon as isize);
        self.control(
            parent,
            "STATIC",
            "THROW / CONTROL CENTER",
            390,
            0,
            [24, 24, 600, 28],
        );
        self.control(
            parent,
            "STATIC",
            "Native Rust. No web runtime. No background monitoring.",
            391,
            0,
            [24, 57, 600, 25],
        );
        let startup = match startup_enabled() {
            Ok(enabled) => enabled,
            Err(error) => {
                message(parent, &error, true);
                DestroyWindow(parent);
                self.settings_hwnd = null_mut();
                return;
            }
        };
        for (id, label, checked, y) in [
            (
                LITE,
                "Lite mode — index up to 5,000 apps (Standard: 20,000)",
                self.settings.lite,
                101,
            ),
            (
                HIDE_START,
                "Start with the launcher hidden",
                self.settings.hide_on_startup,
                135,
            ),
            (
                HIDE_FOCUS,
                "Hide the launcher when it loses focus",
                self.settings.hide_on_focus_loss,
                169,
            ),
            (
                CALCULATOR,
                "Calculator (= 12 * 3)",
                self.settings.calculator,
                203,
            ),
            (
                WEB,
                "Web search (? search terms)",
                self.settings.web_search,
                237,
            ),
            (
                HISTORY,
                "Remember launch frequency locally (maximum 100 apps)",
                self.settings.remember_history,
                271,
            ),
            (STARTUP, "Launch at Windows sign-in", startup, 305),
        ] {
            let control = self.control(
                parent,
                "BUTTON",
                label,
                id,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                [24, y, 600, 28],
            );
            SendMessageW(
                control,
                BM_SETCHECK,
                if checked {
                    BST_CHECKED as usize
                } else {
                    BST_UNCHECKED as usize
                },
                0,
            );
        }
        for (id, label, x, width) in [
            (HOTKEY, "Shortcut", 24, 220),
            (THEME, "Appearance", 268, 150),
            (LIMIT, "Results", 442, 150),
        ] {
            self.control(parent, "STATIC", label, id + 20, 0, [x, 351, width, 24]);
            self.control(
                parent,
                "COMBOBOX",
                "",
                id,
                WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
                [x, 380, width, 150],
            );
        }
        for (id, values, selected) in [
            (
                HOTKEY,
                vec!["Ctrl + Alt + Space", "Alt + Space", "Ctrl + Shift + Space"],
                self.settings.hotkey as usize,
            ),
            (
                THEME,
                vec!["Ink + orange", "Paper + orange"],
                self.settings.theme as usize,
            ),
            (
                LIMIT,
                vec!["4", "6", "8", "10", "12"],
                (self.settings.max_results - 4) / 2,
            ),
        ] {
            let control = GetDlgItem(parent, id as i32);
            for value in values {
                SendMessageW(control, CB_ADDSTRING, 0, wide(value).as_ptr() as isize);
            }
            SendMessageW(control, CB_SETCURSEL, selected, 0);
        }
        self.control(
            parent,
            "BUTTON",
            "Import preferences",
            IMPORT,
            WS_TABSTOP,
            [24, 438, 180, 32],
        );
        self.control(
            parent,
            "BUTTON",
            "Clear launch history",
            CLEAR,
            WS_TABSTOP,
            [218, 438, 180, 32],
        );
        self.control(
            parent,
            "BUTTON",
            "Open profile",
            FOLDER,
            WS_TABSTOP,
            [412, 438, 180, 32],
        );
        self.control(
            parent,
            "BUTTON",
            "Measure memory now",
            MEMORY,
            WS_TABSTOP,
            [24, 488, 205, 32],
        );
        self.memory_label = self.control(
            parent,
            "STATIC",
            "Measured only on request; 50 MB is not guaranteed.",
            392,
            0,
            [24, 532, 600, 44],
        );
        SendMessageW(self.memory_label, WM_SETFONT, self.small_font as usize, 1);
        self.control(
            parent,
            "STATIC",
            "Rust edition 0.1 • .NET plugins are not loaded. Existing profiles stay untouched.",
            393,
            0,
            [24, 584, 600, 40],
        );
        self.control(
            parent,
            "BUTTON",
            "Save changes",
            SAVE,
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            [412, 626, 180, 34],
        );
        ShowWindow(parent, SW_SHOW);
        SetForegroundWindow(parent);
    }

    unsafe fn save_settings(&mut self) -> Result<(), String> {
        let parent = self.settings_hwnd;
        let checked =
            |id| SendMessageW(GetDlgItem(parent, id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let mut next = self.settings.clone();
        next.lite = checked(LITE as i32);
        next.hide_on_startup = checked(HIDE_START as i32);
        next.hide_on_focus_loss = checked(HIDE_FOCUS as i32);
        next.calculator = checked(CALCULATOR as i32);
        next.web_search = checked(WEB as i32);
        next.remember_history = checked(HISTORY as i32);
        if !next.remember_history {
            next.history.clear();
        }
        next.hotkey = match SendMessageW(GetDlgItem(parent, HOTKEY as i32), CB_GETCURSEL, 0, 0) {
            0 => Hotkey::CtrlAltSpace,
            1 => Hotkey::AltSpace,
            2 => Hotkey::CtrlShiftSpace,
            _ => return Err("Select a shortcut.".into()),
        };
        next.theme = match SendMessageW(GetDlgItem(parent, THEME as i32), CB_GETCURSEL, 0, 0) {
            0 => Theme::Ink,
            1 => Theme::Light,
            _ => return Err("Select an appearance.".into()),
        };
        let selected = SendMessageW(GetDlgItem(parent, LIMIT as i32), CB_GETCURSEL, 0, 0);
        if !(0..=4).contains(&selected) {
            return Err("Select a result limit.".into());
        }
        next.max_results = 4 + selected as usize * 2;
        next.validate()?;
        let old = self.settings.clone();
        let old_startup = startup_enabled()?;
        let new_startup = checked(STARTUP as i32);
        if next.hotkey != old.hotkey {
            UnregisterHotKey(self.hwnd, HOTKEY_ID);
            if !register_hotkey(self.hwnd, next.hotkey) {
                register_hotkey(self.hwnd, old.hotkey);
                return Err("This shortcut is in use by another app. Choose another one.".into());
            }
        }
        if let Err(error) = set_startup(new_startup, self.portable)
            .and_then(|_| next.save(&self.root.join("settings.json")))
        {
            let rollback = set_startup(old_startup, self.portable);
            if next.hotkey != old.hotkey {
                UnregisterHotKey(self.hwnd, HOTKEY_ID);
                register_hotkey(self.hwnd, old.hotkey);
            }
            return Err(format!(
                "{error}{}",
                rollback
                    .err()
                    .map(|e| format!("\nStartup rollback also failed: {e}"))
                    .unwrap_or_default()
            ));
        }
        self.settings = next;
        self.rebuild_brushes();
        // Geometry remains bounded even when more results are requested.
        self.entries.truncate(self.limit());
        self.refresh();
        let lite_changed = old.lite != self.settings.lite;
        DestroyWindow(self.settings_hwnd);
        self.settings_hwnd = null_mut();
        InvalidateRect(self.hwnd, null(), 1);
        if lite_changed {
            self.reindex();
        }
        Ok(())
    }

    unsafe fn import(&mut self) -> Result<(), String> {
        self.popup = true;
        let picked = pick_import(self.settings_hwnd);
        self.popup = false;
        let Some(path) = picked? else {
            return Ok(());
        };
        let bytes = read_bounded(&path, 1024 * 1024).map_err(|e| e.to_string())?;
        let mut next = self.settings.clone();
        import_preferences(&bytes, &mut next)?;
        if MessageBoxW(self.settings_hwnd,
            wide("Copy only hide-on-startup and hide-on-focus-loss preferences?\n\nLite will remain enabled. Your current Rust settings will be backed up. No plugins, commands, hotkeys or paths will be imported. The source file stays untouched.").as_ptr(),
            wide("Import preferences").as_ptr(), MB_YESNO | MB_ICONQUESTION) != IDYES {
            return Ok(());
        }
        let destination = self.root.join("settings.json");
        if destination.exists() {
            let backup = self.root.join(format!(
                "settings-before-import-{}.json",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|e| e.to_string())?
                    .as_nanos()
            ));
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(backup)
                .map_err(|e| e.to_string())?;
            use std::io::Write;
            file.write_all(&read_bounded(&destination, 64 * 1024).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        next.save(&destination)?;
        self.settings = next;
        DestroyWindow(self.settings_hwnd);
        self.settings_hwnd = null_mut();
        self.show_settings();
        self.entries.truncate(self.limit());
        self.refresh();
        self.reindex();
        Ok(())
    }

    unsafe fn tray_menu(&mut self) {
        self.popup = true;
        let menu = CreatePopupMenu();
        for (id, title) in [
            (OPEN, "Open Throw"),
            (SETTINGS_MENU, "Control Center"),
            (REINDEX, "Rebuild app index"),
            (QUIT, "Quit"),
        ] {
            AppendMenuW(menu, MF_STRING, id, wide(title).as_ptr());
        }
        let mut cursor = zeroed();
        GetCursorPos(&mut cursor);
        SetForegroundWindow(self.hwnd);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            0,
            self.hwnd,
            null(),
        );
        DestroyMenu(menu);
        self.popup = false;
        match command as usize {
            OPEN => self.show(),
            SETTINGS_MENU => self.show_settings(),
            REINDEX => self.reindex(),
            QUIT => {
                DestroyWindow(self.hwnd);
            }
            _ => (),
        }
    }

    unsafe fn paint(&self, hwnd: HWND) {
        visuals::paint(self, hwnd);
    }

    unsafe fn draw_row(&self, item: &DRAWITEMSTRUCT) {
        visuals::draw_row(self, item);
    }

    unsafe fn dispatch(&mut self, hwnd: HWND, msg: u32, w: usize, l: isize) -> Option<isize> {
        match msg {
            WM_SIZE if hwnd == self.hwnd => {
                if !self.edit.is_null() {
                    self.layout();
                }
                Some(0)
            }
            WM_PAINT => {
                self.paint(hwnd);
                Some(0)
            }
            WM_ERASEBKGND => Some(1),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORLISTBOX | WM_CTLCOLOREDIT => {
                let dc = w as HDC;
                SetTextColor(dc, self.text_color());
                SetBkMode(dc, TRANSPARENT as i32);
                let brush = if msg == WM_CTLCOLOREDIT {
                    self.field
                } else {
                    self.background
                };
                let mut log: LOGBRUSH = zeroed();
                GetObjectW(
                    brush,
                    size_of::<LOGBRUSH>() as i32,
                    (&mut log as *mut LOGBRUSH).cast(),
                );
                SetBkColor(dc, log.lbColor);
                Some(brush as isize)
            }
            WM_DRAWITEM if w == RESULTS => {
                self.draw_row(&*(l as *const DRAWITEMSTRUCT));
                Some(1)
            }
            WM_DRAWITEM if w == SETTINGS_BUTTON => {
                visuals::draw_settings_button(self, &*(l as *const DRAWITEMSTRUCT));
                Some(1)
            }
            ICONS_READY => {
                if let Some(icons) = self.icons.as_mut() {
                    icons.drain();
                }
                self.request_result_icons();
                InvalidateRect(self.list, null(), 0);
                Some(0)
            }
            WM_COMMAND => {
                let id = w & 0xffff;
                let event = (w >> 16) as u32;
                match id {
                    SEARCH if event == EN_CHANGE => self.refresh(),
                    RESULTS if event == LBN_DBLCLK => self.activate(),
                    SETTINGS_BUTTON => self.show_settings(),
                    SAVE => {
                        if let Err(e) = self.save_settings() {
                            message(self.settings_hwnd, &e, true);
                        }
                    }
                    IMPORT => {
                        if let Err(e) = self.import() {
                            message(self.settings_hwnd, &e, true);
                        }
                    }
                    MEMORY => match memory_report() {
                        Ok(report) => {
                            SetWindowTextW(self.memory_label, wide(report).as_ptr());
                        }
                        Err(e) => message(self.settings_hwnd, &e, true),
                    },
                    FOLDER => {
                        if let Err(e) = open(self.settings_hwnd, self.root.as_os_str()) {
                            message(self.settings_hwnd, &e, true);
                        }
                    }
                    CLEAR => {
                        if MessageBoxW(
                            self.settings_hwnd,
                            wide("Clear the Rust launch history? This cannot be undone.").as_ptr(),
                            wide("Clear history").as_ptr(),
                            MB_YESNO | MB_ICONQUESTION,
                        ) == IDYES
                        {
                            let mut next = self.settings.clone();
                            next.history.clear();
                            match next.save(&self.root.join("settings.json")) {
                                Ok(()) => {
                                    self.settings = next;
                                    self.refresh();
                                    message(self.settings_hwnd, "Launch history cleared.", false);
                                }
                                Err(e) => message(self.settings_hwnd, &e, true),
                            }
                        }
                    }
                    _ => (),
                }
                Some(0)
            }
            WM_HOTKEY => {
                if IsWindowVisible(self.hwnd) != 0 {
                    ShowWindow(self.hwnd, SW_HIDE);
                } else {
                    self.show();
                }
                Some(0)
            }
            INDEX_READY => {
                self.index_ready();
                Some(0)
            }
            TRAY => {
                match l as u32 {
                    WM_LBUTTONUP | WM_LBUTTONDBLCLK => self.show(),
                    WM_RBUTTONUP | WM_CONTEXTMENU => self.tray_menu(),
                    _ => (),
                }
                Some(0)
            }
            WM_ACTIVATE if hwnd == self.hwnd => {
                if w & 0xffff == WA_INACTIVE as usize
                    && self.settings.hide_on_focus_loss
                    && self.settings_hwnd.is_null()
                    && !self.popup
                {
                    ShowWindow(hwnd, SW_HIDE);
                }
                Some(0)
            }
            WM_CLOSE => {
                if hwnd == self.settings_hwnd {
                    DestroyWindow(hwnd);
                    self.settings_hwnd = null_mut();
                } else {
                    ShowWindow(hwnd, SW_HIDE);
                }
                Some(0)
            }
            WM_DESTROY if hwnd == self.hwnd => {
                remove_tray(self.hwnd);
                UnregisterHotKey(self.hwnd, HOTKEY_ID);
                PostQuitMessage(0);
                Some(0)
            }
            _ => None,
        }
    }
}

fn rgb(r: u32, g: u32, b: u32) -> u32 {
    r | (g << 8) | (b << 16)
}

unsafe fn register_hotkey(hwnd: HWND, hotkey: Hotkey) -> bool {
    let modifiers = match hotkey {
        Hotkey::CtrlAltSpace => MOD_CONTROL | MOD_ALT,
        Hotkey::AltSpace => MOD_ALT,
        Hotkey::CtrlShiftSpace => MOD_CONTROL | MOD_SHIFT,
    };
    RegisterHotKey(hwnd, HOTKEY_ID, modifiers | MOD_NOREPEAT, VK_SPACE as u32) != 0
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, w: usize, l: isize) -> isize {
    // Destruction can be synchronous inside an App callback. It must run even
    // when the callback owns the state borrow, or Quit would leave a hidden process.
    if msg == WM_DESTROY && MAIN_WINDOW.with(|window| window.get()) == hwnd {
        remove_tray(hwnd);
        UnregisterHotKey(hwnd, HOTKEY_ID);
        PostQuitMessage(0);
        return 0;
    }
    // Win32 may synchronously re-enter during SetWindowText, modal dialogs etc.
    // Never alias &mut App. Nested messages take the normal default path.
    let result = APP.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return None;
        };
        slot.as_mut().and_then(|app| app.dispatch(hwnd, msg, w, l))
    });
    result.unwrap_or_else(|| DefWindowProcW(hwnd, msg, w, l))
}

unsafe extern "system" fn edit_proc(
    hwnd: HWND,
    msg: u32,
    w: usize,
    l: isize,
    subclass: usize,
    _: usize,
) -> isize {
    if msg == WM_SETFOCUS || msg == WM_KILLFOCUS {
        InvalidateRect(GetParent(hwnd), null(), 0);
    }
    if msg == WM_KEYDOWN {
        if w == VK_RETURN as usize
            || w == VK_ESCAPE as usize
            || w == VK_UP as usize
            || w == VK_DOWN as usize
        {
            APP.with(|cell| {
                if let Ok(mut slot) = cell.try_borrow_mut() {
                    if let Some(app) = slot.as_mut() {
                        if w == VK_RETURN as usize {
                            app.activate();
                        } else if w == VK_ESCAPE as usize {
                            ShowWindow(app.hwnd, SW_HIDE);
                        } else {
                            let current = SendMessageW(app.list, LB_GETCURSEL, 0, 0);
                            let max = app.rows.len().saturating_sub(1) as isize;
                            let next = if w == VK_DOWN as usize {
                                (current + 1).min(max)
                            } else {
                                (current - 1).max(0)
                            };
                            SendMessageW(app.list, LB_SETCURSEL, next as usize, 0);
                        }
                    }
                }
            });
            return 0;
        }
    }
    if msg == WM_CHAR && [VK_RETURN as usize, VK_ESCAPE as usize].contains(&w) {
        return 0;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(edit_proc), subclass);
    }
    DefSubclassProc(hwnd, msg, w, l)
}

unsafe fn tray(hwnd: HWND, icon: HICON) -> Result<(), String> {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = TRAY;
    data.hIcon = icon;
    let tip = wide("Throw Launcher • Rust • Ctrl+Alt+Space");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    if Shell_NotifyIconW(NIM_ADD, &data) == 0 {
        Err("Cannot create the system tray icon.".into())
    } else {
        Ok(())
    }
}

unsafe fn remove_tray(hwnd: HWND) {
    let mut data: NOTIFYICONDATAW = zeroed();
    data.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
    data.hWnd = hwnd;
    data.uID = 1;
    Shell_NotifyIconW(NIM_DELETE, &data);
}

pub fn show_error(error: &str) {
    unsafe {
        message(null_mut(), error, true);
    }
}

pub fn run() -> Result<(), String> {
    unsafe { run_native() }
}

unsafe fn run_native() -> Result<(), String> {
    let _shell = ShellApartment::initialize()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| !["--portable", "--background", "--smoke-test"].contains(&a.as_str()))
    {
        return Err("Supported options: --portable, --background, --smoke-test".into());
    }
    let smoke = args.iter().any(|a| a == "--smoke-test");
    let portable = args.iter().any(|a| a == "--portable");
    // An independent singleton; the WPF and Flow applications are untouched.
    let mutex = CreateMutexW(
        null(),
        0,
        wide(if smoke {
            "Local\\ThrowRustSmoke"
        } else {
            "Local\\ThrowLauncherRust"
        })
        .as_ptr(),
    );
    if mutex.is_null() {
        return Err("Cannot create launcher singleton.".into());
    }
    if GetLastError() == ERROR_ALREADY_EXISTS {
        CloseHandle(mutex);
        let existing = FindWindowW(wide(MAIN_CLASS).as_ptr(), null());
        if !existing.is_null() {
            ShowWindow(existing, SW_SHOWNORMAL);
            SetForegroundWindow(existing);
        }
        return Ok(());
    }
    let root = if smoke {
        let path = std::env::temp_dir().join(format!("ThrowRustSmoke-{}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        path
    } else {
        profile(portable)?
    };
    let settings = Settings::load(&root.join("settings.json"))?;
    let instance = GetModuleHandleW(null());
    let scale = (windows_sys::Win32::UI::HiDpi::GetDpiForSystem() as f64 / 96.0).max(1.0);
    for name in [MAIN_CLASS, SETTINGS_CLASS] {
        let class_name = wide(name);
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: class_name.as_ptr(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..zeroed()
        };
        if RegisterClassW(&class) == 0 {
            CloseHandle(mutex);
            return Err("Cannot register native windows.".into());
        }
    }
    let icon = LoadImageW(
        instance,
        1usize as *const u16,
        IMAGE_ICON,
        32,
        32,
        LR_DEFAULTCOLOR,
    ) as HICON;
    let font = |height: i32, weight| {
        CreateFontW(
            -(height as f64 * scale).round() as i32,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            OUT_DEFAULT_PRECIS as u32,
            CLIP_DEFAULT_PRECIS as u32,
            CLEARTYPE_QUALITY as u32,
            DEFAULT_PITCH as u32,
            wide("Segoe UI").as_ptr(),
        )
    };
    let mut app = App {
        hwnd: null_mut(),
        settings_hwnd: null_mut(),
        edit: null_mut(),
        list: null_mut(),
        status: null_mut(),
        memory_label: null_mut(),
        instance,
        icon,
        scale,
        font: font(17, FW_NORMAL as i32),
        small_font: font(12, FW_NORMAL as i32),
        heading_font: font(29, FW_BOLD as i32),
        input_font: font(21, FW_NORMAL as i32),
        icons: None,
        background: null_mut(),
        field: null_mut(),
        selected: null_mut(),
        settings,
        root: root.clone(),
        entries: vec![],
        rows: vec![],
        worker: None,
        reindex_again: false,
        portable,
        popup: false,
    };
    app.rebuild_brushes();
    let mut work: RECT = zeroed();
    if SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0) == 0 {
        work.right = GetSystemMetrics(SM_CXSCREEN);
        work.bottom = GetSystemMetrics(SM_CYSCREEN);
    }
    let width = app
        .px(760)
        .min((work.right - work.left - app.px(24)).max(app.px(360)));
    let height = app
        .px(716)
        .min((work.bottom - work.top - app.px(24)).max(app.px(360)));
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - height) / 3).max(0);
    app.hwnd = CreateWindowExW(
        WS_EX_APPWINDOW,
        wide(MAIN_CLASS).as_ptr(),
        wide("Throw Launcher • Rust").as_ptr(),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN,
        x,
        y,
        width,
        height,
        null_mut(),
        null_mut(),
        instance,
        null(),
    );
    if app.hwnd.is_null() {
        CloseHandle(mutex);
        return Err("Cannot create the launcher window.".into());
    }
    app.icons = Some(IconCache::new(app.hwnd, app.settings.lite));
    SendMessageW(app.hwnd, WM_SETICON, ICON_BIG as usize, icon as isize);
    SendMessageW(app.hwnd, WM_SETICON, ICON_SMALL as usize, icon as isize);
    let hwnd = app.hwnd;
    MAIN_WINDOW.with(|window| window.set(hwnd));
    APP.with(|cell| *cell.borrow_mut() = Some(app));
    let mut setup = APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.create_controls()?;
        if smoke {
            if app.icon.is_null() { return Err("Embedded Throw logo is missing.".into()); }
            SetWindowTextW(app.edit, wide("= (12 + 3) * 2").as_ptr());
            app.refresh();
            if app.rows.first().map(|r| r.title.as_str()) != Some("30") ||
                SendMessageW(app.list, LB_GETCOUNT, 0, 0) != 1 {
                return Err("Native calculator/search list smoke test failed.".into());
            }
            let report = memory_report()?;
            app.show_settings();
            if app.settings_hwnd.is_null() || GetDlgItem(app.settings_hwnd, SAVE as i32).is_null() {
                return Err("Native settings smoke test failed.".into());
            }
            // Persist, reload and compare the actual Windows profile write path.
            app.settings.save(&root.join("settings.json"))?;
            if Settings::load(&root.join("settings.json"))? != app.settings {
                return Err("Windows settings roundtrip failed.".into());
            }
            std::fs::write(root.join("smoke-result.txt"), report).map_err(|e| e.to_string())?;
            DestroyWindow(app.settings_hwnd); app.settings_hwnd = null_mut();
            // Real Shell icons, but no applications are executed by the test.
            let windows = PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot is missing")?);
            app.entries = vec![
                Entry { name: "Notepad".into(), path: windows.join("System32/notepad.exe") },
                Entry { name: "File Explorer".into(), path: windows.join("explorer.exe") },
                Entry { name: "Throw Launcher".into(), path: std::env::current_exe().map_err(|e| e.to_string())? },
            ];
            app.settings.hide_on_focus_loss = false;
            SetWindowTextW(app.edit, wide("").as_ptr());
            app.refresh();
            app.show();
            return Ok(());
        }
        if !register_hotkey(app.hwnd, app.settings.hotkey) {
            message(app.hwnd, "Your shortcut is already in use. Throw will stay open; choose a free shortcut in Settings. You can also open Throw from its tray icon.", false);
        }
        tray(app.hwnd, app.icon)?;
        let cache = load_index(&app.root.join("apps.json"), app.limit());
        match cache {
            Ok(entries) => app.entries = entries,
            Err(error) => message(app.hwnd, &format!("{error}\nThe app index will be rebuilt."), false),
        }
        app.refresh();
        app.reindex();
        if !app.settings.hide_on_startup && !args.iter().any(|a| a == "--background") { app.show(); }
        Ok::<_, String>(())
    });
    if smoke {
        // Smoke mode never registers startup, hotkeys, tray or launches apps.
        if setup.is_ok() {
            setup = snapshot::verify_and_capture(&root);
        }
        APP.with(|cell| {
            if let Some(app) = cell.borrow().as_ref() {
                DestroyWindow(app.hwnd);
            }
        });
        if setup.is_ok() {
            let output = std::env::current_exe()
                .map_err(|e| e.to_string())?
                .parent()
                .ok_or("Missing executable folder")?
                .join("smoke-result.txt");
            std::fs::copy(root.join("smoke-result.txt"), output).map_err(|e| e.to_string())?;
        }
        let _ = std::fs::remove_dir_all(&root);
    } else if setup.is_ok() {
        let mut msg = zeroed();
        loop {
            let result = GetMessageW(&mut msg, null_mut(), 0, 0);
            if result == 0 {
                break;
            }
            if result == -1 {
                return Err("Windows message loop failed.".into());
            }
            // IsDialogMessage dispatches synchronously: release state first.
            let settings_window = APP.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|app| app.settings_hwnd)
                    .unwrap_or(null_mut())
            });
            let handled =
                !settings_window.is_null() && IsDialogMessageW(settings_window, &msg) != 0;
            if !handled {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    } else {
        DestroyWindow(hwnd);
    }
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().take() {
            for object in [
                app.font,
                app.small_font,
                app.heading_font,
                app.input_font,
                app.background,
                app.field,
                app.selected,
            ] {
                DeleteObject(object);
            }
            if !app.icon.is_null() {
                DestroyIcon(app.icon);
            }
        }
    });
    CloseHandle(mutex);
    setup
}
