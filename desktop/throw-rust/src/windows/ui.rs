use super::icons::{IconCache, ICONS_READY};
use super::platform::*;
mod actions;
mod command_icons;

mod placement;
mod settings;
mod snapshot;
mod visuals;
mod web_settings;
mod web_smoke;
use std::{
    cell::{Cell, RefCell},
    mem::{size_of, zeroed},
    path::PathBuf,
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
};
use throw_launcher::{
    bookmarks::{self, Bookmark},
    config::{import_preferences, read_bounded, Hotkey, Settings, Theme},
    files::{self, FileEntry},
    folders::{self, Folder},
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

const FILES_READY: u32 = WM_APP + 5;
const FIT_RESULTS: u32 = WM_APP + 4;
const SHOW_REQUEST: u32 = WM_APP + 6;
const APPLY_LAUNCHER_DPI: u32 = WM_APP + 7;
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
const WEB_MANAGER: usize = 323;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    static MAIN_WINDOW: Cell<HWND> = const { Cell::new(null_mut()) };
    static RESULT_WHEEL_REMAINDER: Cell<i32> = const { Cell::new(0) };
}

struct IndexResult {
    apps: Scan,
    folders: folders::Scan,
    bookmarks: bookmarks::Scan,
}

struct App {
    hwnd: HWND,
    settings_hwnd: HWND,
    web_hwnd: HWND,
    center: settings::Center,
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
    folders: Vec<Folder>,
    bookmarks: Vec<Bookmark>,
    browser_root: Option<PathBuf>,
    bookmark_status: String,
    bookmark_permission: Arc<AtomicBool>,
    rows: Vec<ResultRow>,
    worker: Option<Receiver<Result<IndexResult, String>>>,
    reindex_again: bool,
    portable: bool,
    popup: bool,
    context: Option<actions::Panel>,
    file_worker: Option<Receiver<(String, Result<Vec<FileEntry>, String>)>>,
    file_query: String,
    file_entries: Vec<FileEntry>,
    file_error: Option<String>,
    file_pending: bool,
    shown_query: String,
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
            rgb(239, 241, 247)
        } else {
            rgb(28, 31, 38)
        }
    }

    fn muted(&self) -> u32 {
        if self.settings.theme == Theme::Ink {
            rgb(153, 159, 173)
        } else {
            rgb(115, 121, 133)
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
            rgb(29, 30, 35)
        } else {
            rgb(247, 248, 250)
        });
        self.field = CreateSolidBrush(if ink {
            rgb(38, 39, 46)
        } else {
            rgb(255, 255, 255)
        });
        self.selected = CreateSolidBrush(if ink {
            rgb(48, 57, 74)
        } else {
            rgb(230, 237, 249)
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
            [32, 34, 580, 30],
        );
        SendMessageW(self.edit, EM_SETLIMITTEXT, 512, 0);
        SendMessageW(self.edit, WM_SETFONT, self.input_font as usize, 1);
        // The hint is painted by the subclass, never stored in EDIT text.
        // Native focused cue banners can clear undo when a cut empties EDIT.
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
                | LBS_NOTIFY as u32
                | LBS_HASSTRINGS as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            [20, 176, 704, 448],
        );
        if SetWindowSubclass(self.list, Some(edit_proc), 2, 0) == 0 {
            return Err("Cannot attach result keyboard controls.".into());
        }
        SendMessageW(self.list, LB_SETITEMHEIGHT, 0, self.px(64) as isize);
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
        unsafe fn place(hwnd: HWND, x: i32, y: i32, width: i32, height: i32, _: i32) {
            let mut old: RECT = zeroed();
            GetWindowRect(hwnd, &mut old);
            MapWindowPoints(
                null_mut(),
                GetParent(hwnd),
                (&mut old as *mut RECT).cast(),
                2,
            );
            // Repositioning an EDIT during an edit notification can disrupt its
            // native undo bookkeeping. Height-only launcher changes need not
            // touch the search field at all.
            if old.left != x
                || old.top != y
                || old.right - old.left != width
                || old.bottom - old.top != height
            {
                MoveWindow(hwnd, x, y, width, height, 0);
            }
        }
        let mut client: RECT = zeroed();
        GetClientRect(self.hwnd, &mut client);
        let width = client.right;
        let height = client.bottom;
        place(
            self.edit,
            self.px(32),
            self.px(34),
            (width - self.px(100)).max(1),
            self.px(29),
            0,
        );
        place(
            GetDlgItem(self.hwnd, SETTINGS_BUTTON as i32),
            width - self.px(120),
            self.px(86),
            self.px(104),
            self.px(24),
            0,
        );
        place(
            self.list,
            self.px(16),
            self.px(116),
            (width - self.px(32)).max(1),
            (height - self.px(158)).max(self.px(64)),
            0,
        );
        place(
            self.status,
            self.px(20),
            height - self.px(30),
            if width >= self.px(520) {
                (width - self.px(420)).max(1)
            } else {
                width - self.px(40)
            },
            self.px(22),
            0,
        );
    }

    fn request_result_icons(&mut self) {
        let top = unsafe { SendMessageW(self.list, LB_GETTOPINDEX, 0, 0).max(0) as usize };
        if let Some(icons) = self.icons.as_mut() {
            icons.set_lite(self.settings.lite);
            icons.drain();
            icons.request(self.rows.iter().skip(top).take(6).filter_map(|row| {
                match row.action {
                    Action::App(i) => self
                        .entries
                        .get(i)
                        .filter(|e| e.valid())
                        .map(|e| e.path.clone()),
                    Action::Folder(ref path) | Action::File(ref path) => Some(path.clone()),
                    _ => None,
                }
            }));
        }
    }

    unsafe fn set_status(&self, message: &str) {
        SetWindowTextW(self.status, wide(message).as_ptr());
    }

    unsafe fn refresh(&mut self) {
        if self.context.is_some() {
            self.refresh_context();
            return;
        }
        // Owner-draw callbacks must not paint while App is mutably borrowed.
        // Batch list/geometry changes, then invalidate for the next message turn.
        SendMessageW(self.list, WM_SETREDRAW, 0, 0);
        let mut text = [0u16; 513];
        let count = GetWindowTextW(self.edit, text.as_mut_ptr(), text.len() as i32);
        let text = String::from_utf16_lossy(&text[..count.max(0) as usize]);
        // Preserve the selected item when asynchronous file results arrive.
        let keep_selection = text == self.shown_query && !self.rows.is_empty();
        let selected = SendMessageW(self.list, LB_GETCURSEL, 0, 0);
        let selected_path = if keep_selection && selected >= 0 {
            self.rows
                .get(selected as usize)
                .map(|row| row.detail.clone())
        } else {
            None
        };
        let old_top = SendMessageW(self.list, LB_GETTOPINDEX, 0, 0).max(0);
        self.shown_query = text.clone();
        let terms = if self.settings.web_search
            && throw_launcher::web::match_provider(&text, &self.settings.web_providers).is_some()
        {
            String::new()
        } else {
            files::terms(&text).unwrap_or("").to_string()
        };
        if terms != self.file_query {
            self.file_query = terms;
            self.file_entries.clear();
            self.file_error = None;
            self.file_pending = !self.file_query.is_empty();
            KillTimer(self.hwnd, FILE_TIMER);
            if self.file_pending && SetTimer(self.hwnd, FILE_TIMER, 150, None) == 0 {
                self.file_pending = false;
                self.file_error = Some("Cannot schedule file search. Try typing again.".into());
            }
        }
        self.rows = search::query_sources(
            &text,
            &self.entries,
            &self.folders,
            &self.bookmarks,
            &self.settings,
        );
        files::merge(
            &mut self.rows,
            &self.file_entries,
            self.settings.max_results,
            text.trim().starts_with("file:"),
        );
        // EN_CHANGE is synchronous inside WM_CUT/PASTE/UNDO. Resize after that
        // native operation finishes, rather than re-entering it mid-mutation.
        PostMessageW(self.hwnd, FIT_RESULTS, 0, 0);
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
            let selection = if keep_selection {
                selected_path.and_then(|path| self.rows.iter().position(|row| row.detail == path))
            } else {
                Some(0)
            };
            SendMessageW(self.list, LB_SETCURSEL, selection.unwrap_or(usize::MAX), 0);
            SendMessageW(
                self.list,
                LB_SETTOPINDEX,
                if keep_selection { old_top as usize } else { 0 },
                0,
            );
        }
        self.request_result_icons();
        if let Some(error) = &self.file_error {
            self.set_status(error);
        } else if self.file_pending {
            self.set_status("Searching indexed files… Apps can be opened now.");
        } else if self.worker.is_none() {
            self.set_status(&if self.rows.is_empty() && !text.trim().is_empty() {
                "No matches. Files need 2+ characters and an indexed folder; try a folder name or local path too.".into()
            } else {
                format!(
                    "{} apps · {} folders · {}",
                    self.entries.len(),
                    self.folders.len(),
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
        SendMessageW(self.list, WM_SETREDRAW, 1, 0);
        if self.rows.is_empty() {
            ShowWindow(self.list, SW_HIDE);
        }
        InvalidateRect(self.list, null(), 0);
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }

    unsafe fn fit_results(&self) {
        let work = placement::work_area(self.hwnd);
        let mut rect: RECT = zeroed();
        GetWindowRect(self.hwnd, &mut rect);
        let rows = if self.context.is_some() { 5 } else { 3 };
        let mut size = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: self.px(158 + rows as i32 * 64),
        };
        AdjustWindowRectEx(
            &mut size,
            WS_POPUP | WS_BORDER | WS_CLIPCHILDREN,
            0,
            WS_EX_TOOLWINDOW,
        );
        let height = (size.bottom - size.top).min(work.bottom - work.top - self.px(24));
        let y = rect
            .top
            .clamp(work.top, (work.bottom - height).max(work.top));
        SetWindowPos(
            self.hwnd,
            null_mut(),
            rect.left.clamp(
                work.left,
                (work.right - (rect.right - rect.left)).max(work.left),
            ),
            y,
            rect.right - rect.left,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW | SWP_NOCOPYBITS,
        );
        // A resize can re-enter while App is borrowed; lay out explicitly too.
        self.layout();
    }

    unsafe fn show(&self) {
        // Capture the user's active window BEFORE activating Throw. Using the
        // launcher's previous position would reopen it on the wrong monitor.
        let work = placement::work_area(GetForegroundWindow());
        let bounds = placement::centered(
            work,
            self.px(640),
            self.px(if self.context.is_some() { 480 } else { 352 }),
            self.px(12),
        );
        SetWindowPos(
            self.hwnd,
            null_mut(),
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        self.layout();
        ShowWindow(self.hwnd, SW_SHOWNORMAL);
        if SetForegroundWindow(self.hwnd) == 0 {
            // The hotkey normally grants foreground rights. If Windows kept
            // another input queue active, join it only for this explicit open.
            let foreground = GetForegroundWindow();
            let thread = GetWindowThreadProcessId(foreground, null_mut());
            let current = GetCurrentThreadId();
            if thread != 0 && thread != current && AttachThreadInput(current, thread, 1) != 0 {
                BringWindowToTop(self.hwnd);
                SetForegroundWindow(self.hwnd);
                SetFocus(self.edit);
                AttachThreadInput(current, thread, 0);
            }
        }
        SetFocus(self.edit);
        SendMessageW(self.edit, EM_SETSEL, 0, -1);
    }

    unsafe fn launcher_dpi_changed(&mut self, dpi: u32) {
        self.scale = dpi.max(96) as f64 / 96.0;
        let next = [
            (17, FW_NORMAL),
            (12, FW_NORMAL),
            (29, FW_BOLD),
            (21, FW_NORMAL),
        ]
        .map(|(size, weight)| {
            CreateFontW(
                -(size as f64 * self.scale).round() as i32,
                0,
                0,
                0,
                weight as i32,
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
        });
        if next.iter().any(|font| font.is_null()) {
            for font in next {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
            return;
        }
        let old = [
            self.font,
            self.small_font,
            self.heading_font,
            self.input_font,
        ];
        [
            self.font,
            self.small_font,
            self.heading_font,
            self.input_font,
        ] = next;
        for (control, font) in [
            (self.edit, self.input_font),
            (self.list, self.font),
            (self.status, self.small_font),
        ] {
            SendMessageW(control, WM_SETFONT, font as usize, 0);
        }
        for font in old {
            DeleteObject(font);
        }
        SendMessageW(self.list, LB_SETITEMHEIGHT, 0, self.px(64) as isize);
        let work = placement::work_area(self.hwnd);
        let bounds = placement::centered(
            work,
            self.px(640),
            self.px(if self.context.is_some() { 480 } else { 352 }),
            self.px(12),
        );
        SetWindowPos(
            self.hwnd,
            null_mut(),
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        self.layout();
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }

    fn row_identity(&self, row: &ResultRow) -> String {
        match &row.action {
            Action::App(i) => format!(
                "app:{}",
                self.entries
                    .get(*i)
                    .map(|e| e.path.to_string_lossy())
                    .unwrap_or_default()
            ),
            Action::Folder(path) => format!("folder:{}", path.display()),
            action => format!("{action:?}"),
        }
    }

    unsafe fn selected_identity(&self) -> Option<String> {
        let selected = SendMessageW(self.list, LB_GETCURSEL, 0, 0);
        self.rows
            .get(selected as usize)
            .map(|row| self.row_identity(row))
    }

    unsafe fn restore_selection(&mut self, identity: Option<String>) {
        if let Some(i) = identity.and_then(|identity| {
            self.rows
                .iter()
                .position(|row| self.row_identity(row) == identity)
        }) {
            SendMessageW(self.list, LB_SETCURSEL, i, 0);
            SendMessageW(self.list, LB_SETTOPINDEX, i.saturating_sub(2), 0);
            self.request_result_icons();
            InvalidateRect(self.list, null(), 0);
        }
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
        let folder_roots = match folder_roots() {
            Ok(roots) => roots,
            Err(error) => {
                self.set_status(&error);
                return;
            }
        };
        self.set_status("Finding apps and folders…");
        let (sender, receiver) = mpsc::channel();
        let limit = self.limit();
        let cache = self.root.join("apps.json");
        let folder_cache = self.root.join("folders.json");
        let lite = self.settings.lite;
        let bookmarks_enabled = self.settings.bookmarks;
        let bookmark_permission = Arc::clone(&self.bookmark_permission);
        let browser_root = self.browser_root.clone();
        let hwnd = self.hwnd as usize;
        self.worker = Some(receiver);
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(|| {
                let scan = index::scan(&roots, limit);
                save_index(&cache, &scan.entries)?;
                let folder_scan = folders::scan(&folder_roots, lite);
                folders::save(&folder_cache, &folder_scan.entries)?;
                let bookmark_scan = if !bookmarks_enabled
                    || !bookmark_permission.load(Ordering::Acquire)
                {
                    bookmarks::Scan::default()
                } else if let Some(root) = browser_root {
                    bookmarks::discover_while(&root, || bookmark_permission.load(Ordering::Acquire))
                } else {
                    bookmarks::Scan {
                        errors: vec![
                            "LOCALAPPDATA is unavailable; bookmark discovery could not run.".into(),
                        ],
                        ..Default::default()
                    }
                };
                Ok::<_, String>(IndexResult {
                    apps: scan,
                    folders: folder_scan,
                    bookmarks: bookmark_scan,
                })
            })
            .unwrap_or_else(|_| Err("App/folder indexing failed unexpectedly.".into()));
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
            Some(Ok(mut result)) => {
                let selected = self.selected_identity();
                if let Some(icons) = self.icons.as_mut() {
                    icons.clear();
                }
                // Settings may have changed while the background scan ran.
                result.apps.entries.truncate(self.limit());
                result.folders.entries.truncate(if self.settings.lite {
                    folders::LITE_LIMIT
                } else {
                    folders::STANDARD_LIMIT
                });
                self.entries = result.apps.entries;
                self.folders = result.folders.entries;
                if self.settings.bookmarks {
                    self.bookmark_status = format!(
                        "{} bookmarks · {} unsafe entries skipped{}{}",
                        result.bookmarks.entries.len(),
                        result.bookmarks.skipped,
                        if result.bookmarks.limited {
                            " · scan limit reached"
                        } else {
                            ""
                        },
                        result
                            .bookmarks
                            .errors
                            .first()
                            .map(|e| format!(" · {} error(s): {e}", result.bookmarks.errors.len()))
                            .unwrap_or_default()
                    );
                    self.bookmarks = result.bookmarks.entries;
                } else {
                    self.bookmarks.clear();
                    self.bookmark_status = "Bookmarks off. Browser files are not read.".into();
                }
                self.update_bookmark_status();
                self.refresh();
                self.restore_selection(selected);
                if result.apps.unreadable > 0
                    || result.apps.truncated
                    || result.folders.limited
                    || result.folders.unreadable > 0
                {
                    self.set_status(&format!(
                        "{} apps · {} folders · {} skipped{}",
                        self.entries.len(),
                        self.folders.len(),
                        result.apps.unreadable + result.folders.unreadable,
                        if result.apps.truncated || result.folders.limited {
                            " · bounded scan"
                        } else {
                            ""
                        }
                    ));
                }
            }
            Some(Err(error)) => {
                if self.settings.bookmarks {
                    self.bookmark_status = format!("Bookmark refresh could not finish: {error}");
                    self.update_bookmark_status();
                }
                self.set_status(&error);
            }
            None => {
                self.set_status(
                    "Search index worker did not return a result. Try > Rebuild search index.",
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
        if let Action::Context(id) = row.action {
            actions::perform(self, id);
            return;
        }
        let result = match row.action {
            Action::Context(_) => unreachable!(),
            Action::File(path) => {
                validate_file(&path).and_then(|_| open(self.hwnd, path.as_os_str()))
            }
            Action::System(command) => {
                if !super::system_actions::confirm(self.hwnd, command) {
                    self.set_status("Cancelled — no system action was performed.");
                    return;
                }
                super::system_actions::execute(self.hwnd, command, true)
            }
            Action::Utility(command) => {
                use throw_launcher::commands::UtilityCommand::*;
                match command {
                    MeasureMemory => {
                        match memory_report() {
                            Ok(report) => message(self.hwnd, &report, false),
                            Err(error) => message(self.hwnd, &error, true),
                        }
                        return;
                    }
                    LightTheme | DarkTheme => {
                        let mut next = self.settings.clone();
                        next.theme = if command == LightTheme {
                            Theme::Light
                        } else {
                            Theme::Ink
                        };
                        if let Err(error) = next.save(&self.root.join("settings.json")) {
                            message(self.hwnd, &error, true);
                        } else {
                            self.settings = next;
                            self.sync_settings_appearance();
                            self.rebuild_brushes();
                            RedrawWindow(
                                self.hwnd,
                                null(),
                                null_mut(),
                                RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
                            );
                            self.set_status("Appearance saved.");
                        }
                        return;
                    }
                    _ => super::system_actions::utility(self.hwnd, command, &self.root),
                }
            }
            Action::Folder(path) => {
                if Folder::new(path.clone()).is_none() || !path.is_dir() {
                    Err("This folder is unavailable or no longer exists.".into())
                } else {
                    open(self.hwnd, path.as_os_str())
                }
            }
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
        if let Err(error) = self.open_control_center() {
            self.close_control_center();
            message(self.hwnd, &error, true);
        }
    }

    unsafe fn save_settings(&mut self) -> Result<(), String> {
        let parent = self.settings_hwnd;
        let checked =
            |id| SendMessageW(GetDlgItem(parent, id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let mut next = self.settings_draft()?;
        if !next.remember_history {
            next.history.clear();
        }
        next.validate()?;
        let old = self.settings.clone();
        let old_startup = startup_enabled()?;
        let new_startup = checked(STARTUP as i32);
        let startup_changed = new_startup != old_startup;
        if next.hotkey != old.hotkey {
            UnregisterHotKey(self.hwnd, HOTKEY_ID);
            if !register_hotkey(self.hwnd, next.hotkey) {
                let restored = register_hotkey(self.hwnd, old.hotkey);
                return Err(format!(
                    "This shortcut is in use by another app. Choose another one.{}",
                    if restored {
                        ""
                    } else {
                        "\nThe previous shortcut could not be restored. Open Throw from the tray and choose a free shortcut."
                    }
                ));
            }
        }
        if let Err(error) = (if startup_changed {
            set_startup(new_startup, self.portable)
        } else {
            Ok(())
        })
        .and_then(|_| next.save(&self.root.join("settings.json")))
        {
            let rollback = if startup_changed {
                set_startup(old_startup, self.portable)
            } else {
                Ok(())
            };
            let mut hotkey_error = "";
            if next.hotkey != old.hotkey {
                UnregisterHotKey(self.hwnd, HOTKEY_ID);
                if !register_hotkey(self.hwnd, old.hotkey) {
                    hotkey_error = "\nThe previous shortcut could not be restored. Open Throw from the tray and choose a free shortcut.";
                }
            }
            return Err(format!(
                "{error}{}{hotkey_error}",
                rollback
                    .err()
                    .map(|e| format!("\nStartup rollback also failed: {e}"))
                    .unwrap_or_default()
            ));
        }
        self.settings = next;
        self.center.startup = new_startup;
        if old.web_search != self.settings.web_search {
            self.sync_web_search_switch();
        }
        self.rebuild_brushes();
        self.apply_settings_theme();
        // Geometry remains bounded even when more results are requested.
        self.entries.truncate(self.limit());
        self.folders.truncate(if self.settings.lite {
            folders::LITE_LIMIT
        } else {
            folders::STANDARD_LIMIT
        });
        self.refresh();
        let lite_changed = old.lite != self.settings.lite;
        self.settings_notice("Changes saved.", false);
        InvalidateRect(self.hwnd, null(), 1);
        if lite_changed {
            self.reindex();
        }
        Ok(())
    }

    unsafe fn import(&mut self) -> Result<(), String> {
        if !self.confirm_discard_settings() {
            return Ok(());
        }
        self.popup = true;
        let picked = pick_import(self.settings_hwnd);
        self.popup = false;
        let Some(path) = picked? else {
            return Ok(());
        };
        self.import_settings_file(&path)
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
            SHOW_REQUEST => {
                self.show();
                Some(0)
            }
            APPLY_LAUNCHER_DPI => {
                self.launcher_dpi_changed(w as u32);
                Some(0)
            }
            FIT_RESULTS => {
                self.fit_results();
                RedrawWindow(
                    self.hwnd,
                    null(),
                    null_mut(),
                    RDW_INVALIDATE | RDW_ALLCHILDREN,
                );
                Some(0)
            }
            WM_NCHITTEST if hwnd == self.hwnd => {
                let mut point = POINT {
                    x: l as i16 as i32,
                    y: (l >> 16) as i16 as i32,
                };
                ScreenToClient(hwnd, &mut point);
                let mut client: RECT = zeroed();
                GetClientRect(hwnd, &mut client);
                if point.y < self.px(14) || point.y >= client.bottom - self.px(36) {
                    Some(HTCAPTION as isize)
                } else {
                    None
                }
            }
            WM_SIZE if hwnd == self.hwnd => {
                if !self.edit.is_null() {
                    self.layout();
                }
                Some(0)
            }
            WM_PAINT => {
                if hwnd == self.settings_hwnd {
                    self.paint_control_center();
                } else {
                    self.paint(hwnd);
                }
                Some(0)
            }
            WM_PRINTCLIENT if hwnd == self.settings_hwnd => {
                self.paint_control_center_dc(w as HDC);
                Some(0)
            }
            WM_DPICHANGED if hwnd == self.settings_hwnd => {
                self.settings_dpi_changed((w & 0xffff) as u32, &*(l as *const RECT));
                Some(0)
            }
            WM_DRAWITEM if w == settings::NAV => {
                self.draw_settings_navigation(&*(l as *const DRAWITEMSTRUCT));
                Some(1)
            }
            WM_ERASEBKGND => Some(1),
            DM_GETDEFID if hwnd == self.settings_hwnd => {
                Some(((DC_HASDEFID as usize) << 16 | SAVE) as isize)
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORLISTBOX | WM_CTLCOLOREDIT => {
                if hwnd == self.settings_hwnd {
                    return Some(self.settings_control_color(w as HDC, l as HWND));
                }
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
                    WEB_MANAGER => self.show_web_settings(),
                    id if web_settings::is_command(id) => {
                        if let Err(error) = self.web_command(id, event) {
                            message(self.web_hwnd, &error, true);
                        }
                    }
                    settings::NAV if event == LBN_SELCHANGE => {
                        let page = SendMessageW(
                            GetDlgItem(hwnd, settings::NAV as i32),
                            LB_GETCURSEL,
                            0,
                            0,
                        );
                        if page >= 0 {
                            self.select_settings_section(page as usize);
                        }
                    }
                    settings::CANCEL | 2 if hwnd == self.settings_hwnd => {
                        if self.confirm_discard_settings() {
                            self.close_control_center();
                        }
                    }
                    SEARCH if event == EN_CHANGE => self.refresh(),
                    RESULTS if event == LBN_DBLCLK => self.activate(),
                    SETTINGS_BUTTON => {
                        if self.context.is_some() {
                            self.close_context();
                        } else {
                            self.show_settings();
                        }
                    }
                    SAVE => {
                        if let Err(e) = self.save_settings() {
                            self.settings_notice(&e, true);
                            message(self.settings_hwnd, &e, true);
                        }
                    }
                    IMPORT => {
                        if let Err(e) = self.import() {
                            self.settings_notice(&e, true);
                            message(self.settings_hwnd, &e, true);
                        }
                    }
                    MEMORY => match memory_report() {
                        Ok(report) => {
                            SetWindowTextW(self.memory_label, wide(report).as_ptr());
                        }
                        Err(e) => {
                            self.settings_notice(&e, true);
                            message(self.settings_hwnd, &e, true);
                        }
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
                    _ if hwnd == self.settings_hwnd
                        && (event == BN_CLICKED || event == CBN_SELCHANGE) =>
                    {
                        if self.settings_dirty() {
                            self.settings_notice("Unsaved changes", false);
                        } else {
                            self.settings_notice("No unsaved changes", false);
                        }
                    }
                    _ => (),
                }
                Some(0)
            }
            WM_HOTKEY => {
                if GetForegroundWindow() == self.hwnd && IsWindowVisible(self.hwnd) != 0 {
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
            FILES_READY => {
                self.files_ready();
                Some(0)
            }
            WM_TIMER if w == FILE_TIMER => {
                self.start_file_search();
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
                    && self.web_hwnd.is_null()
                    && !self.popup
                {
                    ShowWindow(hwnd, SW_HIDE);
                }
                Some(0)
            }
            WM_CLOSE => {
                if hwnd == self.web_hwnd {
                    DestroyWindow(hwnd);
                    self.web_hwnd = null_mut();
                } else if hwnd == self.settings_hwnd {
                    if self.confirm_discard_settings() {
                        self.close_control_center();
                    }
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

    unsafe fn start_file_search(&mut self) {
        KillTimer(self.hwnd, FILE_TIMER);
        // Single flight: typing cannot create a thread or queued request per key.
        // The completion handler schedules only the latest desired query.
        if self.file_worker.is_some() || !self.file_pending || self.file_query.is_empty() {
            return;
        }
        let query = self.file_query.clone();
        let hwnd = self.hwnd as usize;
        let (sender, receiver) = mpsc::sync_channel(1);
        self.file_worker = Some(receiver);
        std::thread::spawn(move || {
            let result =
                std::panic::catch_unwind(|| super::files::search(&query)).unwrap_or_else(|_| {
                    Err("File search failed unexpectedly. Apps still work.".into())
                });
            if sender.send((query, result)).is_ok() {
                PostMessageW(hwnd as HWND, FILES_READY, 0, 0);
            }
        });
    }

    unsafe fn files_ready(&mut self) {
        let completed = self.file_worker.take().and_then(|r| r.try_recv().ok());
        if let Some((query, result)) = completed {
            if query == self.file_query {
                self.file_pending = false;
                match result {
                    Ok(entries) => self.file_entries = entries,
                    Err(error) => self.file_error = Some(error),
                }
                self.refresh();
            } else {
                self.start_file_search();
            }
        } else {
            self.file_pending = false;
            self.file_error = Some(
                "File search worker did not return results. Type a new query to retry.".into(),
            );
            self.refresh();
        }
    }

    unsafe fn import_settings_file(&mut self, path: &std::path::Path) -> Result<(), String> {
        let bytes = read_bounded(&path, 1024 * 1024).map_err(|e| e.to_string())?;
        let mut next = self.settings.clone();
        import_preferences(&bytes, &mut next)?;
        if MessageBoxW(self.settings_hwnd,
            wide("Copy only hide-on-startup and hide-on-focus-loss preferences?\n\nLite will remain enabled. Your current Rust settings will be backed up. No plugins, commands, hotkeys or paths will be imported. The source file stays untouched.").as_ptr(),
            wide("Import preferences").as_ptr(), MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION) != IDYES {
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
        self.close_control_center();
        self.show_settings();
        self.settings_notice(
            "Preferences imported. Previous settings backed up if present.",
            false,
        );
        self.entries.truncate(self.limit());
        self.refresh();
        self.reindex();
        Ok(())
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
    if msg == WM_DPICHANGED && MAIN_WINDOW.with(|window| window.get()) == hwnd {
        // Moving a window synchronously sends this while App may be borrowed.
        // Queue the font/layout update rather than losing it to default handling.
        PostMessageW(hwnd, APPLY_LAUNCHER_DPI, w & 0xffff, 0);
        return 0;
    }
    // Fixed-layout Settings can be larger than a small CI desktop while testing
    // high DPI. Process this even during synchronous SetWindowPos callbacks,
    // when App is already borrowed; the default screen-sized cap clips controls.
    if msg == WM_GETMINMAXINFO && MAIN_WINDOW.with(|window| window.get()) != hwnd {
        let info = &mut *(l as *mut MINMAXINFO);
        info.ptMaxTrackSize = POINT { x: 8192, y: 8192 };
        return 0;
    }
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
    if subclass == 1 && msg == WM_PAINT {
        let result = DefSubclassProc(hwnd, msg, w, l);
        if GetWindowTextLengthW(hwnd) == 0 {
            APP.with(|cell| {
                if let Ok(slot) = cell.try_borrow() {
                    if let Some(app) = slot.as_ref() {
                        visuals::draw_search_hint(app, hwnd);
                    }
                }
            });
        }
        return result;
    }
    if msg == WM_CONTEXTMENU && subclass == 2 {
        actions::context_menu(hwnd, l);
        return 0;
    }
    if msg == WM_KEYDOWN && GetKeyState(VK_CONTROL as i32) < 0 && GetKeyState(VK_MENU as i32) >= 0 {
        if subclass == 1 && w == b'K' as usize && GetKeyState(VK_SHIFT as i32) >= 0 {
            // Release the shared borrow before opening the panel mutably.
            let list = APP.with(|cell| {
                cell.try_borrow()
                    .ok()
                    .and_then(|slot| slot.as_ref().map(|app| app.list))
            });
            if let Some(list) = list {
                actions::context_menu(list, -1);
            }
            return 0;
        }
        let edit = APP.with(|cell| {
            cell.try_borrow()
                .ok()
                .and_then(|slot| slot.as_ref().map(|app| app.edit))
        });
        if edit.is_some_and(|edit| actions::text_shortcut(edit, w)) {
            return 0;
        }
    }
    // TranslateMessage emits a control character after Ctrl+A/C/K/X/V/Z. Do not
    // let the original EDIT process the command twice or insert a control glyph.
    if msg == WM_CHAR && [1, 3, 11, 22, 24, 26].contains(&w) {
        return 0;
    }
    if msg == WM_SYSKEYDOWN && GetKeyState(VK_MENU as i32) < 0 && (0x31..=0x39).contains(&w) {
        APP.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut() {
                if let Some(app) = slot.as_mut() {
                    let index = w - 0x31;
                    if index < app.rows.len() {
                        SendMessageW(app.list, LB_SETCURSEL, index, 0);
                        app.activate();
                    }
                }
            }
        });
        return 0;
    }
    if msg == WM_SYSCHAR && (0x31..=0x39).contains(&w) {
        return 0;
    }
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
                            if app.context.is_some() {
                                app.close_context();
                            } else {
                                ShowWindow(app.hwnd, SW_HIDE);
                            }
                        } else {
                            let current = SendMessageW(app.list, LB_GETCURSEL, 0, 0);
                            let max = app.rows.len().saturating_sub(1) as isize;
                            let next = if w == VK_DOWN as usize {
                                (current + 1).min(max)
                            } else {
                                (current - 1).max(0)
                            };
                            SendMessageW(app.list, LB_SETCURSEL, next as usize, 0);
                            app.request_result_icons();
                            InvalidateRect(app.list, null(), 0);
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
    let result = if subclass == 2 && msg == WM_MOUSEWHEEL {
        // No native scrollbar chrome, but preserve wheel/trackpad scrolling.
        // Accumulate high-resolution deltas and respect Windows' scroll setting.
        let delta = ((w >> 16) as u16) as i16 as i32;
        let steps = RESULT_WHEEL_REMAINDER.with(|remainder| {
            let total = remainder.get() + delta;
            remainder.set(total % 120);
            total / 120
        });
        if steps != 0 {
            let mut lines = 3u32;
            SystemParametersInfoW(
                SPI_GETWHEELSCROLLLINES,
                0,
                (&mut lines as *mut u32).cast(),
                0,
            );
            let count = SendMessageW(hwnd, LB_GETCOUNT, 0, 0).max(0);
            let top = SendMessageW(hwnd, LB_GETTOPINDEX, 0, 0).max(0);
            let mut rect: RECT = zeroed();
            GetClientRect(hwnd, &mut rect);
            let height = SendMessageW(hwnd, LB_GETITEMHEIGHT, 0, 0).max(1);
            let visible = ((rect.bottom - rect.top) as isize / height).max(1);
            let lines = if lines == u32::MAX {
                visible
            } else {
                lines.min(100) as isize
            };
            let next = (top - steps as isize * lines).clamp(0, (count - visible).max(0));
            SendMessageW(hwnd, LB_SETTOPINDEX, next as usize, 0);
        }
        0
    } else {
        DefSubclassProc(hwnd, msg, w, l)
    };
    if subclass == 2 && [WM_MOUSEWHEEL, WM_VSCROLL, WM_KEYUP].contains(&msg) {
        APP.with(|cell| {
            if let Ok(mut slot) = cell.try_borrow_mut() {
                if let Some(app) = slot.as_mut() {
                    app.request_result_icons();
                    InvalidateRect(app.list, null(), 0);
                }
            }
        });
    }
    result
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
    windows_sys::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
        windows_sys::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    );
    let _shell = ShellApartment::initialize()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| {
        ![
            "--portable",
            "--background",
            "--smoke-test",
            "--file-search-benchmark",
        ]
        .contains(&a.as_str())
    }) {
        return Err(
            "Supported options: --portable, --background, --smoke-test, --file-search-benchmark"
                .into(),
        );
    }
    let benchmark = args.iter().any(|a| a == "--file-search-benchmark");
    let smoke = benchmark || args.iter().any(|a| a == "--smoke-test");
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
            let mut process = 0;
            GetWindowThreadProcessId(existing, &mut process);
            AllowSetForegroundWindow(process);
            PostMessageW(existing, SHOW_REQUEST, 0, 0);
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
        web_hwnd: null_mut(),
        center: settings::Center::default(),
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
        bookmark_status: if settings.bookmarks {
            "Bookmarks waiting for the background scan.".into()
        } else {
            "Bookmarks off. Browser files are not read.".into()
        },
        bookmark_permission: Arc::new(AtomicBool::new(settings.bookmarks)),
        settings,
        root: root.clone(),
        entries: vec![],
        folders: vec![],
        bookmarks: vec![],
        browser_root: if smoke {
            Some(root.join("browser-fixtures"))
        } else {
            std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
        },
        rows: vec![],
        worker: None,
        file_worker: None,
        file_query: String::new(),
        file_entries: vec![],
        file_error: None,
        file_pending: false,
        shown_query: String::new(),
        reindex_again: false,
        portable,
        popup: false,
        context: None,
    };
    app.rebuild_brushes();
    let mut work: RECT = zeroed();
    if SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut work as *mut RECT).cast(), 0) == 0 {
        work.right = GetSystemMetrics(SM_CXSCREEN);
        work.bottom = GetSystemMetrics(SM_CYSCREEN);
    }
    let width = app
        .px(640)
        .min((work.right - work.left - app.px(24)).max(app.px(360)));
    let height = app
        .px(716)
        .min((work.bottom - work.top - app.px(24)).max(app.px(360)));
    let x = work.left + ((work.right - work.left - width) / 2).max(0);
    let y = work.top + ((work.bottom - work.top - height) / 3).max(0);
    app.hwnd = CreateWindowExW(
        // A launcher is a transient tray utility, not a taskbar application.
        // Keep activation/keyboard focus: do not use WS_EX_NOACTIVATE.
        WS_EX_TOOLWINDOW,
        wide(MAIN_CLASS).as_ptr(),
        wide("Throw Launcher").as_ptr(),
        WS_POPUP | WS_BORDER | WS_CLIPCHILDREN,
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
    // Cosmetic enhancements on Windows 11; earlier Windows keeps a native border.
    let corners = 2i32; // DWMWCP_ROUND
    windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
        app.hwnd,
        33,
        (&corners as *const i32).cast(),
        size_of::<i32>() as u32,
    );
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
            let file = root.join("native-file-smoke.txt");
            std::fs::write(&file, b"File result binding").map_err(|e| e.to_string())?;
            validate_file(&file)?;
            SetWindowTextW(app.edit, wide("file: native-file-smoke").as_ptr());
            app.file_query = "native-file-smoke".into();
            app.file_entries = vec![FileEntry { path: file.clone() }];
            app.refresh();
            if app.rows.len() != 1
                || !matches!(&app.rows[0].action, Action::File(path) if path == &file)
                || SendMessageW(app.list, LB_GETCOUNT, 0, 0) != 1
                || SendMessageW(app.list, LB_GETCURSEL, 0, 0) != 0
            {
                return Err("Native file result/list selection smoke test failed.".into());
            }
            std::fs::remove_file(&file).map_err(|e| e.to_string())?;
            if validate_file(&file).is_ok() {
                return Err("Deleted file was still allowed to open.".into());
            }
            let report = if benchmark {
                super::benchmark::run(&root)?
            } else {
                memory_report()?
            };
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
            app.close_control_center();
            // Real Shell icons, but no applications are executed by the test.
            let windows = PathBuf::from(std::env::var_os("SystemRoot").ok_or("SystemRoot is missing")?);
            app.entries = [
                Entry { name: "Notepad".into(), path: windows.join("System32").join("notepad.exe") },
                Entry { name: "File Explorer".into(), path: windows.join("explorer.exe") },
                Entry { name: "Command Prompt".into(), path: windows.join("System32").join("cmd.exe") },
            ].into_iter().filter(|entry| entry.path.is_file()).take(2).collect();
            app.entries.push(Entry { name: "Throw Launcher".into(), path: std::env::current_exe().map_err(|e| e.to_string())? });
            for name in ["Documents", "Projects"] {
                let path = root.join(name);
                std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
                app.folders.push(Folder::new(path).ok_or("Invalid folder fixture")?);
            }
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
        match folders::load(&app.root.join("folders.json"), app.settings.lite) {
            Ok(folders) => app.folders = folders,
            Err(error) => message(app.hwnd, &format!("{error}\nThe folder index will be rebuilt."), false),
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
                .join(if benchmark {
                    "file-search-benchmark.txt"
                } else {
                    "smoke-result.txt"
                });
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
            let (settings_window, web_window) = APP.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(|app| (app.settings_hwnd, app.web_hwnd))
                    .unwrap_or((null_mut(), null_mut()))
            });
            let handled = (!web_window.is_null() && IsDialogMessageW(web_window, &msg) != 0)
                || (!settings_window.is_null()
                    && (settings::translate_key(&msg)
                        || IsDialogMessageW(settings_window, &msg) != 0));
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
            app.center.delete_fonts();
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

const FILE_TIMER: usize = 42;
