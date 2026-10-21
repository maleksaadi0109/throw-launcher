//! Focused native editor; persists only validated candidates before changing live state.
use super::*;
use throw_launcher::web::Provider;

pub(super) const BOOKMARKS: usize = 500;
pub(super) const WEB_ENABLED: usize = 501;
pub(super) const PROVIDERS: usize = 502;
pub(super) const NAME: usize = 503;
pub(super) const KEYWORD: usize = 504;
pub(super) const TEMPLATE: usize = 505;
pub(super) const ENABLED: usize = 506;
pub(super) const NEW: usize = 507;
pub(super) const STORE: usize = 508;
pub(super) const DELETE: usize = 509;
pub(super) const APPLY: usize = 510;
pub(super) const RESCAN: usize = 511;
const STATUS: usize = 512;

pub(super) fn is_command(id: usize) -> bool {
    (BOOKMARKS..=RESCAN).contains(&id)
}

unsafe fn checked(parent: HWND, id: usize) -> bool {
    SendMessageW(GetDlgItem(parent, id as i32), BM_GETCHECK, 0, 0) == BST_CHECKED as isize
}

unsafe fn set_checked(parent: HWND, id: usize, value: bool) {
    SendMessageW(
        GetDlgItem(parent, id as i32),
        BM_SETCHECK,
        if value {
            BST_CHECKED as usize
        } else {
            BST_UNCHECKED as usize
        },
        0,
    );
}

unsafe fn text(parent: HWND, id: usize, max: usize) -> Result<String, String> {
    let control = GetDlgItem(parent, id as i32);
    let len = GetWindowTextLengthW(control).max(0) as usize;
    if len > max {
        return Err("Provider field is too long.".into());
    }
    let mut buf = vec![0; len + 1];
    let count = GetWindowTextW(control, buf.as_mut_ptr(), buf.len() as i32).max(0) as usize;
    String::from_utf16(&buf[..count]).map_err(|_| "Provider text contains invalid Unicode.".into())
}

impl App {
    pub(super) unsafe fn show_web_settings(&mut self) {
        if !self.web_hwnd.is_null() {
            ShowWindow(self.web_hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(self.web_hwnd);
            return;
        }
        self.web_hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(SETTINGS_CLASS).as_ptr(),
            wide("Throw · Bookmarks & web providers").as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            self.px(680),
            self.px(690),
            self.hwnd,
            null_mut(),
            self.instance,
            null(),
        );
        let parent = self.web_hwnd;
        if parent.is_null() {
            message(self.hwnd, "Cannot open web settings.", true);
            return;
        }
        for (id, label, value, y) in [
            (
                BOOKMARKS,
                "Search browser bookmarks (off by default)",
                self.settings.bookmarks,
                20,
            ),
            (
                WEB_ENABLED,
                "Enable keyword web searches",
                self.settings.web_search,
                54,
            ),
        ] {
            self.control(
                parent,
                "BUTTON",
                label,
                id,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32,
                [24, y, 620, 28],
            );
            set_checked(parent, id, value);
        }
        self.control(
            parent,
            "BUTTON",
            "Apply switches",
            APPLY,
            WS_TABSTOP,
            [24, 94, 180, 32],
        );
        self.control(
            parent,
            "BUTTON",
            "Refresh bookmarks / index",
            RESCAN,
            WS_TABSTOP,
            [222, 94, 280, 32],
        );
        self.control(parent, "STATIC", "Read-only: Chrome, Edge, Brave and Chromium; Default / Profile 1–8.\nNo Firefox, Safari, custom profiles, cloud sync or bookmark editing.", 513, 0, [24, 138, 620, 44]);
        self.control(
            parent,
            "STATIC",
            &self.bookmark_status,
            STATUS,
            0,
            [24, 190, 620, 64],
        );
        self.control(
            parent,
            "STATIC",
            "Provider (select to edit, or choose New)",
            514,
            0,
            [24, 266, 620, 24],
        );
        self.control(
            parent,
            "COMBOBOX",
            "",
            PROVIDERS,
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            [24, 294, 454, 230],
        );
        self.control(
            parent,
            "BUTTON",
            "New",
            NEW,
            WS_TABSTOP,
            [496, 294, 148, 28],
        );
        for (id, label, y, max) in [
            (NAME, "Name", 338, 80),
            (KEYWORD, "Keyword (e.g. g, wiki, ?)", 382, 16),
            (TEMPLATE, "URL template", 426, 2048),
        ] {
            self.control(parent, "STATIC", label, id + 20, 0, [24, y, 220, 24]);
            let edit = self.control(
                parent,
                "EDIT",
                "",
                id,
                WS_TABSTOP | WS_BORDER | ES_AUTOHSCROLL as u32,
                [250, y, 394, 28],
            );
            SendMessageW(edit, EM_SETLIMITTEXT, max, 0);
        }
        self.control(parent, "STATIC", "Exactly one {query}, e.g. https://example.com/search?q={query}\nTerms are URL-encoded; searches are sent to the chosen provider.", 515, 0, [24, 466, 620, 44]);
        self.control(
            parent,
            "BUTTON",
            "Provider enabled",
            ENABLED,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            [24, 520, 620, 28],
        );
        self.control(
            parent,
            "BUTTON",
            "Save provider",
            STORE,
            WS_TABSTOP,
            [24, 566, 200, 34],
        );
        self.control(
            parent,
            "BUTTON",
            "Delete provider",
            DELETE,
            WS_TABSTOP,
            [244, 566, 200, 34],
        );
        self.control(
            parent,
            "STATIC",
            "Each Save/Delete/Apply is saved immediately. Closing discards field edits.",
            516,
            0,
            [24, 614, 620, 24],
        );
        self.fill_providers(None);
        ShowWindow(parent, SW_SHOW);
        SetForegroundWindow(parent);
    }

    unsafe fn fill_providers(&self, selected: Option<usize>) {
        let combo = GetDlgItem(self.web_hwnd, PROVIDERS as i32);
        SendMessageW(combo, CB_RESETCONTENT, 0, 0);
        for provider in &self.settings.web_providers {
            let title = format!(
                "{} · {}{}",
                provider.keyword,
                provider.name,
                if provider.enabled { "" } else { " (off)" }
            );
            SendMessageW(combo, CB_ADDSTRING, 0, wide(title).as_ptr() as isize);
        }
        SendMessageW(combo, CB_SETCURSEL, selected.unwrap_or(usize::MAX), 0);
        self.load_provider();
    }

    unsafe fn load_provider(&self) {
        let index = SendMessageW(
            GetDlgItem(self.web_hwnd, PROVIDERS as i32),
            CB_GETCURSEL,
            0,
            0,
        );
        let provider = if index >= 0 {
            self.settings.web_providers.get(index as usize)
        } else {
            None
        };
        for (id, value) in [
            (NAME, provider.map(|p| p.name.as_str()).unwrap_or("")),
            (KEYWORD, provider.map(|p| p.keyword.as_str()).unwrap_or("")),
            (
                TEMPLATE,
                provider
                    .map(|p| p.template.as_str())
                    .unwrap_or("https://example.com/search?q={query}"),
            ),
        ] {
            SetWindowTextW(GetDlgItem(self.web_hwnd, id as i32), wide(value).as_ptr());
        }
        set_checked(
            self.web_hwnd,
            ENABLED,
            provider.map(|p| p.enabled).unwrap_or(true),
        );
        EnableWindow(
            GetDlgItem(self.web_hwnd, DELETE as i32),
            i32::from(provider.is_some()),
        );
    }

    unsafe fn persist_web(&mut self, next: Settings) -> Result<(), String> {
        // Do not mutate live state (or lose the old file) on validation/write failure.
        let web_changed = next.web_search != self.settings.web_search;
        next.save(&self.root.join("settings.json"))?;
        self.bookmark_permission
            .store(next.bookmarks, Ordering::Release);
        self.settings = next;
        if web_changed && !self.settings_hwnd.is_null() {
            set_checked(self.settings_hwnd, WEB, self.settings.web_search);
        }
        self.refresh();
        Ok(())
    }

    pub(super) unsafe fn sync_web_search_switch(&self) {
        if !self.web_hwnd.is_null() {
            set_checked(self.web_hwnd, WEB_ENABLED, self.settings.web_search);
        }
    }

    pub(super) unsafe fn save_provider(&mut self) -> Result<(), String> {
        let parent = self.web_hwnd;
        let provider = Provider {
            name: text(parent, NAME, 80)?.trim().into(),
            keyword: text(parent, KEYWORD, 16)?.trim().into(),
            template: text(parent, TEMPLATE, 2048)?.trim().into(),
            enabled: checked(parent, ENABLED),
        };
        let mut next = self.settings.clone();
        let selected = SendMessageW(GetDlgItem(parent, PROVIDERS as i32), CB_GETCURSEL, 0, 0);
        let index = if selected >= 0 {
            let index = selected as usize;
            *next
                .web_providers
                .get_mut(index)
                .ok_or("Select a provider again.")? = provider;
            index
        } else {
            next.web_providers.push(provider);
            next.web_providers.len() - 1
        };
        self.persist_web(next)?;
        self.fill_providers(Some(index));
        Ok(())
    }

    pub(super) unsafe fn delete_provider(&mut self) -> Result<(), String> {
        let selected = SendMessageW(
            GetDlgItem(self.web_hwnd, PROVIDERS as i32),
            CB_GETCURSEL,
            0,
            0,
        );
        let mut next = self.settings.clone();
        if selected < 0 || selected as usize >= next.web_providers.len() {
            return Err("Select a provider to delete.".into());
        }
        next.web_providers.remove(selected as usize);
        self.persist_web(next)?;
        self.fill_providers(None);
        Ok(())
    }

    pub(super) unsafe fn apply_web_switches(&mut self, rescan: bool) -> Result<(), String> {
        let mut next = self.settings.clone();
        next.bookmarks = checked(self.web_hwnd, BOOKMARKS);
        next.web_search = checked(self.web_hwnd, WEB_ENABLED);
        let changed = next.bookmarks != self.settings.bookmarks;
        self.persist_web(next)?;
        if !self.settings.bookmarks {
            self.bookmarks.clear();
            self.bookmark_status = "Bookmarks off. Browser files are not read.".into();
            self.update_bookmark_status();
        } else if changed || rescan {
            self.bookmark_status = "Refreshing bookmarks in the background…".into();
            self.update_bookmark_status();
        }
        if (changed && self.settings.bookmarks) || rescan {
            self.reindex();
        }
        Ok(())
    }

    pub(super) unsafe fn update_bookmark_status(&self) {
        if !self.web_hwnd.is_null() {
            SetWindowTextW(
                GetDlgItem(self.web_hwnd, STATUS as i32),
                wide(&self.bookmark_status).as_ptr(),
            );
        }
    }

    pub(super) unsafe fn web_command(&mut self, id: usize, event: u32) -> Result<(), String> {
        match id {
            PROVIDERS if event == CBN_SELCHANGE => self.load_provider(),
            NEW => self.fill_providers(None),
            STORE => self.save_provider()?,
            DELETE => {
                if MessageBoxW(
                    self.web_hwnd,
                    wide("Delete this web provider? Its keyword shortcut will stop working.")
                        .as_ptr(),
                    wide("Throw · Delete provider").as_ptr(),
                    MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
                ) == IDYES
                {
                    self.delete_provider()?;
                }
            }
            APPLY => self.apply_web_switches(false)?,
            RESCAN => self.apply_web_switches(true)?,
            _ => {}
        }
        Ok(())
    }
}
