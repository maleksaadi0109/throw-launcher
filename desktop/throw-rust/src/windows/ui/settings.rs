//! Native, sectioned Control Center. All pages keep their controls alive so
//! switching sections never discards a draft or changes the persisted profile.
use super::*;
use windows_sys::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SetThreadDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};

pub(super) const NAV: usize = 400;
pub(super) const CANCEL: usize = 401;
pub(super) const TITLE: usize = 402;
const SUBTITLE: usize = 403;
pub(super) const NOTICE: usize = 404;
const ALL: usize = usize::MAX;
const SECTIONS: [(&str, &str); 6] = [
    ("General", "Choose how Throw fits into your day."),
    ("Appearance", "A quiet launcher, in your preferred theme."),
    ("Search", "Control results and built-in search tools."),
    ("Shortcuts", "Keep your hands on the keyboard."),
    ("Privacy", "Your profile and launch history stay local."),
    (
        "Resources",
        "Bounded indexing. Real measurements. Native Rust.",
    ),
];

struct Control {
    hwnd: HWND,
    section: usize,
    rect: [i32; 4],
    font: usize,
    static_painter: Option<Box<StaticPaintData>>,
}

struct StaticPaintData {
    dark: std::cell::Cell<bool>,
    sidebar: bool,
    muted: bool,
    notice: bool,
    error: std::cell::Cell<bool>,
}

pub(super) struct Center {
    pub section: usize,
    pub startup: bool,
    pub scale: f64,
    controls: Vec<Control>,
    fonts: [HFONT; 3],
    error: bool,
    // Preserve a custom, valid result limit until the user actually changes it.
    limits: Vec<usize>,
}

impl Default for Center {
    fn default() -> Self {
        Self {
            section: 0,
            startup: false,
            scale: 1.0,
            controls: vec![],
            fonts: [null_mut(); 3],
            error: false,
            limits: vec![],
        }
    }
}

impl Center {
    pub fn px(&self, value: i32) -> i32 {
        (value as f64 * self.scale).round() as i32
    }

    pub unsafe fn delete_fonts(&self) {
        for font in self.fonts {
            if !font.is_null() {
                DeleteObject(font);
            }
        }
    }

    unsafe fn update_fonts(&mut self) {
        self.delete_fonts();
        for (i, (size, weight)) in [(14, FW_NORMAL), (23, FW_SEMIBOLD), (12, FW_NORMAL)]
            .into_iter()
            .enumerate()
        {
            self.fonts[i] = CreateFontW(
                -self.px(size),
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
            );
        }
        for control in &self.controls {
            SendMessageW(
                control.hwnd,
                WM_SETFONT,
                self.fonts[control.font] as usize,
                1,
            );
        }
    }
}

impl App {
    unsafe fn settings_control(
        &mut self,
        class: &str,
        text: &str,
        id: usize,
        style: u32,
        section: usize,
        rect: [i32; 4],
        font: usize,
    ) -> Result<HWND, String> {
        let [x, y, width, height] = rect.map(|v| self.center.px(v));
        let hwnd = CreateWindowExW(
            0,
            wide(class).as_ptr(),
            wide(text).as_ptr(),
            WS_CHILD | WS_VISIBLE | style,
            x,
            y,
            width,
            height,
            self.settings_hwnd,
            id as HMENU,
            self.instance,
            null(),
        );
        if hwnd.is_null() {
            return Err(format!("Cannot create the Settings control: {text}"));
        }
        SendMessageW(hwnd, WM_SETFONT, self.center.fonts[font] as usize, 0);
        let mut static_painter = if class == "STATIC" {
            Some(Box::new(StaticPaintData {
                dark: std::cell::Cell::new(self.settings.theme == Theme::Ink),
                sidebar: rect[0] < 208,
                muted: font == 2,
                notice: id == NOTICE,
                error: std::cell::Cell::new(false),
            }))
        } else {
            None
        };
        if let Some(painter) = static_painter.as_mut() {
            let ref_data = (&mut **painter as *mut StaticPaintData) as usize;
            if SetWindowSubclass(hwnd, Some(settings_static_proc), 1, ref_data) == 0 {
                return Err(format!("Cannot apply the native text style: {text}"));
            }
        }
        self.center.controls.push(Control {
            hwnd,
            section,
            rect,
            font,
            static_painter,
        });
        Ok(hwnd)
    }

    unsafe fn settings_note(
        &mut self,
        section: usize,
        text: &str,
        y: i32,
        height: i32,
    ) -> Result<(), String> {
        self.settings_control(
            "STATIC",
            text,
            0,
            SS_LEFT as u32 | SS_NOPREFIX as u32,
            section,
            [228, y, 528, height],
            2,
        )?;
        Ok(())
    }

    unsafe fn settings_check(
        &mut self,
        section: usize,
        id: usize,
        text: &str,
        value: bool,
        y: i32,
    ) -> Result<(), String> {
        let hwnd = self.settings_control(
            "BUTTON",
            text,
            id,
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            section,
            [228, y, 528, 34],
            0,
        )?;
        if SetWindowSubclass(hwnd, Some(settings_checkbox_proc), 1, 0) == 0 {
            return Err(format!("Cannot apply the native checkbox style: {text}"));
        }
        SendMessageW(
            hwnd,
            BM_SETCHECK,
            if value { BST_CHECKED } else { BST_UNCHECKED } as usize,
            0,
        );
        Ok(())
    }

    unsafe fn settings_button(
        &mut self,
        section: usize,
        id: usize,
        text: &str,
        rect: [i32; 4],
    ) -> Result<(), String> {
        self.settings_control("BUTTON", text, id, WS_TABSTOP, section, rect, 0)?;
        Ok(())
    }

    unsafe fn settings_combo(
        &mut self,
        section: usize,
        id: usize,
        title: &str,
        values: &[String],
        selected: usize,
        y: i32,
    ) -> Result<(), String> {
        self.settings_control("STATIC", title, 0, 0, section, [224, y, 526, 24], 0)?;
        let hwnd = self.settings_control(
            "COMBOBOX",
            "",
            id,
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            section,
            [224, y + 30, 340, 180],
            0,
        )?;
        for value in values {
            SendMessageW(hwnd, CB_ADDSTRING, 0, wide(value).as_ptr() as isize);
        }
        SendMessageW(hwnd, CB_SETCURSEL, selected, 0);
        Ok(())
    }

    unsafe fn position_settings_footer(&mut self) {
        if self.settings_hwnd.is_null() {
            return;
        }
        let mut client: RECT = zeroed();
        GetClientRect(self.settings_hwnd, &mut client);
        let logical_height = (client.bottom as f64 / self.center.scale).round() as i32;
        let button_y = (logical_height - 48).max(420);
        let scale = self.center.scale;
        for control in &mut self.center.controls {
            match GetDlgCtrlID(control.hwnd) as usize {
                NAV => control.rect = [16, 108, 184, (logical_height - 124).clamp(44, 420)],
                NOTICE => control.rect = [228, button_y + 5, 300, 28],
                CANCEL => control.rect = [532, button_y, 100, 36],
                SAVE => control.rect = [644, button_y, 112, 36],
                _ => continue,
            }
            let [x, y, width, height] = control.rect.map(|v| (v as f64 * scale).round() as i32);
            MoveWindow(control.hwnd, x, y, width, height, 1);
        }
        SendMessageW(
            GetDlgItem(self.settings_hwnd, NAV as i32),
            LB_SETITEMHEIGHT,
            0,
            self.center.px(44) as isize,
        );
    }

    pub(super) unsafe fn open_control_center(&mut self) -> Result<(), String> {
        if !self.settings_hwnd.is_null() {
            self.fit_settings_to_launcher_monitor(GetDpiForWindow(self.hwnd).max(96));
            ShowWindow(self.settings_hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(self.settings_hwnd);
            return Ok(());
        }
        self.center.startup = startup_enabled()?;
        let dpi = GetDpiForWindow(self.hwnd).max(96);
        let (scale, bounds) = self.settings_bounds_for_monitor(dpi);
        self.center.scale = scale;
        self.center.update_fonts();
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        // Mixed-mode DPI: leave the existing launcher awareness unchanged,
        // while Settings handles per-monitor font/geometry changes itself.
        let previous_dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let x = bounds.left;
        let y = bounds.top;
        self.settings_hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            wide(SETTINGS_CLASS).as_ptr(),
            wide("Throw Settings").as_ptr(),
            style,
            x,
            y,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            self.hwnd,
            null_mut(),
            self.instance,
            null(),
        );
        if !previous_dpi.is_null() {
            SetThreadDpiAwarenessContext(previous_dpi);
        }
        if self.settings_hwnd.is_null() {
            return Err("Cannot open Throw Settings.".into());
        }
        SendMessageW(
            self.settings_hwnd,
            WM_SETICON,
            ICON_BIG as usize,
            self.icon as isize,
        );
        self.settings_control("STATIC", "Throw", 0, 0, ALL, [24, 24, 160, 36], 1)?;
        self.settings_control("STATIC", "SETTINGS", 0, 0, ALL, [26, 64, 160, 22], 2)?;
        let nav = self.settings_control(
            "LISTBOX",
            "Settings sections",
            NAV,
            WS_TABSTOP
                | LBS_NOTIFY as u32
                | LBS_HASSTRINGS as u32
                | LBS_OWNERDRAWFIXED as u32
                | LBS_NOINTEGRALHEIGHT as u32,
            ALL,
            [16, 108, 184, 420],
            0,
        )?;
        SendMessageW(nav, LB_SETITEMHEIGHT, 0, self.center.px(44) as isize);
        for (name, _) in SECTIONS {
            SendMessageW(nav, LB_ADDSTRING, 0, wide(name).as_ptr() as isize);
        }
        self.settings_control("STATIC", "", TITLE, 0, ALL, [228, 26, 528, 34], 1)?;
        self.settings_control("STATIC", "", SUBTITLE, 0, ALL, [228, 66, 528, 34], 2)?;

        self.settings_check(
            0,
            STARTUP,
            "Launch at Windows sign-in",
            self.center.startup,
            130,
        )?;
        self.settings_check(
            0,
            HIDE_START,
            "Start with the launcher hidden",
            self.settings.hide_on_startup,
            178,
        )?;
        self.settings_check(
            0,
            HIDE_FOCUS,
            "Hide when the launcher loses focus",
            self.settings.hide_on_focus_loss,
            226,
        )?;
        self.settings_note(0, "Closing the launcher hides it. Use the tray icon or your global shortcut to open it again.", 284, 58)?;

        self.settings_combo(
            1,
            THEME,
            "Theme",
            &["Graphite — Dark".into(), "Pearl — Light".into()],
            self.settings.theme as usize,
            130,
        )?;
        self.settings_note(1, "The theme applies to the launcher and Settings when saved. Appearance commands update both immediately.", 224, 58)?;
        self.settings_note(1, "Three visible result rows, in either theme.\nScroll or use the arrow keys for more results.", 304, 52)?;

        self.settings_check(2, CALCULATOR, "Calculator", self.settings.calculator, 124)?;
        self.settings_note(
            2,
            "Type = (12 + 3) * 2 to calculate and copy the answer.",
            160,
            25,
        )?;
        self.settings_check(2, WEB, "Web search", self.settings.web_search, 202)?;
        self.settings_note(
            2,
            "Type ? followed by a query to search in your browser.",
            230,
            25,
        )?;
        self.center.limits = vec![25, 50, 100];
        if !self.center.limits.contains(&self.settings.max_results) {
            self.center.limits.push(self.settings.max_results);
            self.center.limits.sort_unstable();
        }
        let values = self
            .center
            .limits
            .iter()
            .map(|n| format!("{n} results"))
            .collect::<Vec<_>>();
        let selected = self
            .center
            .limits
            .iter()
            .position(|n| *n == self.settings.max_results)
            .unwrap();
        self.settings_combo(2, LIMIT, "Scrollable result limit", &values, selected, 292)?;
        self.settings_note(2, "App and folder search use the existing local index. Folder selection is not available in this edition yet.", 392, 60)?;
        self.settings_control(
            "BUTTON",
            "Bookmarks & web providers",
            WEB_MANAGER,
            WS_TABSTOP,
            2,
            [228, 466, 420, 32],
            0,
        )?;

        self.settings_combo(
            3,
            HOTKEY,
            "Open Throw",
            &[
                "Ctrl + Alt + Space".into(),
                "Alt + Space".into(),
                "Ctrl + Shift + Space".into(),
            ],
            self.settings.hotkey as usize,
            130,
        )?;
        self.settings_note(3, "LAUNCHER\nUp / Down    Select result          Enter    Open\nAlt + 1–9     Open ranked result    Esc        Back / Hide\nCtrl + K    App / folder actions from search\nCtrl + A / C / X / V / Z    Native text editing", 218, 132)?;
        self.settings_note(3, "SETTINGS\nTab / Shift + Tab    Move between controls\nUp / Down    Switch section when sidebar is focused\nCtrl + 1–6    Jump to section     Ctrl + S    Save\nEsc    Close (asks before discarding changes)", 354, 122)?;

        self.settings_check(
            4,
            HISTORY,
            "Remember launch frequency on this device",
            self.settings.remember_history,
            130,
        )?;
        self.settings_note(4, "At most 100 app counts are kept in your Rust profile.\nTurning this off clears saved counts when you save.", 176, 58)?;
        self.settings_button(4, CLEAR, "Clear launch history…", [228, 254, 240, 36])?;
        self.settings_button(4, FOLDER, "Open Rust profile", [496, 254, 240, 36])?;
        self.settings_button(4, IMPORT, "Import preferences…", [228, 312, 240, 36])?;
        self.settings_note(4, "Import copies only startup visibility and focus behavior, with a backup. Flow and C# profiles remain untouched.\nWeb queries and opened links go to your browser.", 372, 84)?;

        self.settings_check(5, LITE, "Lite mode (recommended)", self.settings.lite, 130)?;
        self.settings_note(5, "Up to 5,000 apps and 10,000 folders, with bounded scans.\nTurning Lite off rebuilds the index with higher limits.", 176, 58)?;
        self.settings_button(5, MEMORY, "Measure memory now", [228, 254, 240, 36])?;
        self.memory_label = self.settings_control(
            "STATIC",
            "Measured only on request. A 50 MB working set is not guaranteed.",
            392,
            0,
            5,
            [228, 302, 528, 56],
            2,
        )?;
        self.settings_note(5, "Rust edition 0.1 · Native Win32 · No web runtime\nOriginal plugin compatibility is deferred; no .NET plugins are loaded.", 378, 72)?;

        self.settings_control(
            "STATIC",
            "No unsaved changes",
            NOTICE,
            0,
            ALL,
            [228, 541, 300, 28],
            2,
        )?;
        self.settings_button(ALL, CANCEL, "Cancel", [532, 536, 100, 36])?;
        self.settings_control(
            "BUTTON",
            "Save changes",
            SAVE,
            WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ALL,
            [644, 536, 112, 36],
            0,
        )?;
        self.position_settings_footer();
        self.select_settings_section(self.center.section);
        self.apply_settings_theme();
        ShowWindow(self.settings_hwnd, SW_SHOW);
        SetForegroundWindow(self.settings_hwnd);
        SetFocus(nav);
        Ok(())
    }

    pub(super) unsafe fn select_settings_section(&mut self, section: usize) {
        if section >= SECTIONS.len() {
            return;
        }
        self.center.section = section;
        for control in &self.center.controls {
            ShowWindow(
                control.hwnd,
                if control.section == ALL || control.section == section {
                    SW_SHOW
                } else {
                    SW_HIDE
                },
            );
        }
        let nav = GetDlgItem(self.settings_hwnd, NAV as i32);
        SendMessageW(nav, LB_SETCURSEL, section, 0);
        SetWindowTextW(
            GetDlgItem(self.settings_hwnd, TITLE as i32),
            wide(SECTIONS[section].0).as_ptr(),
        );
        SetWindowTextW(
            GetDlgItem(self.settings_hwnd, SUBTITLE as i32),
            wide(SECTIONS[section].1).as_ptr(),
        );
        InvalidateRect(self.settings_hwnd, null(), 1);
        InvalidateRect(nav, null(), 1);
    }

    pub(super) unsafe fn settings_draft(&self) -> Result<Settings, String> {
        let parent = self.settings_hwnd;
        let checked =
            |id| SendMessageW(GetDlgItem(parent, id), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let selection = |id| SendMessageW(GetDlgItem(parent, id), CB_GETCURSEL, 0, 0);
        let mut next = self.settings.clone();
        next.lite = checked(LITE as i32);
        next.hide_on_startup = checked(HIDE_START as i32);
        next.hide_on_focus_loss = checked(HIDE_FOCUS as i32);
        next.calculator = checked(CALCULATOR as i32);
        next.web_search = checked(WEB as i32);
        next.remember_history = checked(HISTORY as i32);
        next.hotkey = match selection(HOTKEY as i32) {
            0 => Hotkey::CtrlAltSpace,
            1 => Hotkey::AltSpace,
            2 => Hotkey::CtrlShiftSpace,
            _ => return Err("Select a shortcut in Shortcuts.".into()),
        };
        next.theme = match selection(THEME as i32) {
            0 => Theme::Ink,
            1 => Theme::Light,
            _ => return Err("Select a theme in Appearance.".into()),
        };
        next.max_results = *self
            .center
            .limits
            .get(selection(LIMIT as i32) as usize)
            .ok_or("Select a result limit in Search.")?;
        next.validate()?;
        Ok(next)
    }

    pub(super) unsafe fn settings_dirty(&self) -> bool {
        if self.settings_hwnd.is_null() {
            return false;
        }
        self.settings_draft()
            .map(|draft| draft != self.settings)
            .unwrap_or(true)
            || (SendMessageW(
                GetDlgItem(self.settings_hwnd, STARTUP as i32),
                BM_GETCHECK,
                0,
                0,
            ) == BST_CHECKED as isize)
                != self.center.startup
    }

    pub(super) unsafe fn confirm_discard_settings(&self) -> bool {
        !self.settings_dirty()
            || MessageBoxW(
                self.settings_hwnd,
                wide("Discard your unsaved settings changes?").as_ptr(),
                wide("Unsaved changes — Throw").as_ptr(),
                MB_YESNO | MB_DEFBUTTON2 | MB_ICONQUESTION,
            ) == IDYES
    }

    pub(super) unsafe fn close_control_center(&mut self) {
        if !self.settings_hwnd.is_null() {
            DestroyWindow(self.settings_hwnd);
        }
        self.settings_hwnd = null_mut();
        self.memory_label = null_mut();
        self.center.controls.clear();
        self.center.delete_fonts();
        self.center.fonts = [null_mut(); 3];
    }

    pub(super) unsafe fn settings_notice(&mut self, text: &str, error: bool) {
        self.center.error = error;
        let notice = GetDlgItem(self.settings_hwnd, NOTICE as i32);
        if let Some(control) = self.center.controls.iter_mut().find(|c| c.hwnd == notice) {
            if let Some(painter) = control.static_painter.as_mut() {
                painter.error.set(error);
            }
        }
        SetWindowTextW(notice, wide(text).as_ptr());
        InvalidateRect(notice, null(), 1);
    }

    pub(super) unsafe fn settings_control_color(&self, dc: HDC, hwnd: HWND) -> isize {
        let control = self
            .center
            .controls
            .iter()
            .find(|control| control.hwnd == hwnd);
        let sidebar = control.is_some_and(|control| control.rect[0] < 208);
        let color = if GetDlgCtrlID(hwnd) == NOTICE as i32 && self.center.error {
            if self.settings.theme == Theme::Ink {
                rgb(255, 164, 157)
            } else {
                rgb(170, 40, 35)
            }
        } else if control.is_some_and(|control| control.font == 2) {
            self.muted()
        } else {
            self.text_color()
        };
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        let brush = if sidebar { self.field } else { self.background };
        let mut log: LOGBRUSH = zeroed();
        GetObjectW(
            brush,
            size_of::<LOGBRUSH>() as i32,
            (&mut log as *mut LOGBRUSH).cast(),
        );
        SetBkColor(dc, log.lbColor);
        brush as isize
    }

    pub(super) unsafe fn sync_settings_appearance(&mut self) {
        if self.settings_hwnd.is_null() {
            return;
        }
        SendMessageW(
            GetDlgItem(self.settings_hwnd, THEME as i32),
            CB_SETCURSEL,
            self.settings.theme as usize,
            0,
        );
        self.apply_settings_theme();
        self.settings_notice(
            "Appearance command saved. Other edits are unchanged.",
            false,
        );
    }

    pub(super) unsafe fn apply_settings_theme(&self) {
        if self.settings_hwnd.is_null() {
            return;
        }
        let dark = (self.settings.theme == Theme::Ink) as i32;
        for control in &self.center.controls {
            if let Some(painter) = control.static_painter.as_ref() {
                painter.dark.set(dark != 0);
                if painter.notice {
                    painter.error.set(self.center.error);
                }
            }
        }
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            self.settings_hwnd,
            20,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        );
        for control in &self.center.controls {
            if [
                STARTUP, HIDE_START, HIDE_FOCUS, CALCULATOR, WEB, HISTORY, LITE,
            ]
            .contains(&(GetDlgCtrlID(control.hwnd) as usize))
            {
                SetWindowSubclass(control.hwnd, Some(settings_checkbox_proc), 1, dark as usize);
            }
            SetWindowTheme(
                control.hwnd,
                wide(if dark != 0 {
                    "DarkMode_Explorer"
                } else {
                    "Explorer"
                })
                .as_ptr(),
                null(),
            );
        }
        RedrawWindow(
            self.settings_hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
        );
    }

    pub(super) unsafe fn settings_dpi_changed(&mut self, dpi: u32, rect: &RECT) {
        self.fit_settings_to_monitor(dpi.max(96), Some(rect));
    }

    unsafe fn fit_settings_to_launcher_monitor(&mut self, dpi: u32) {
        self.fit_settings_to_monitor(dpi, None);
    }

    unsafe fn fit_settings_to_monitor(&mut self, dpi: u32, target: Option<&RECT>) {
        let previous_dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let (scale, bounds) = self.settings_bounds_at(dpi, target);
        self.center.scale = scale;
        self.center.update_fonts();
        SetWindowPos(
            self.settings_hwnd,
            null_mut(),
            bounds.left,
            bounds.top,
            bounds.right - bounds.left,
            bounds.bottom - bounds.top,
            SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOREDRAW,
        );
        for control in &self.center.controls {
            let [x, y, w, h] = control.rect.map(|v| self.center.px(v));
            MoveWindow(control.hwnd, x, y, w, h, 1);
        }
        self.position_settings_footer();
        RedrawWindow(
            self.settings_hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_ALLCHILDREN,
        );
        if !previous_dpi.is_null() {
            SetThreadDpiAwarenessContext(previous_dpi);
        }
    }

    unsafe fn settings_bounds_for_monitor(&self, dpi: u32) -> (f64, RECT) {
        self.settings_bounds_at(dpi, None)
    }

    unsafe fn settings_bounds_at(&self, dpi: u32, target: Option<&RECT>) -> (f64, RECT) {
        let previous_dpi = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mut monitor_info: MONITORINFO = zeroed();
        monitor_info.cbSize = size_of::<MONITORINFO>() as u32;
        let monitor = target.map_or_else(
            || MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST),
            |rect| MonitorFromRect(rect, MONITOR_DEFAULTTONEAREST),
        );
        let work = if !monitor.is_null() && GetMonitorInfoW(monitor, &mut monitor_info) != 0 {
            monitor_info.rcWork
        } else {
            let mut fallback: RECT = zeroed();
            SystemParametersInfoW(SPI_GETWORKAREA, 0, (&mut fallback as *mut RECT).cast(), 0);
            fallback
        };
        let work_width = (work.right - work.left).max(1);
        let work_height = (work.bottom - work.top).max(1);
        let dpi_scale = dpi.max(96) as f64 / 96.0;
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_CLIPCHILDREN;
        let mut scale = dpi_scale;
        // Frame thickness is DPI-sized while the content can be proportionally
        // reduced to keep every section and the footer inside the work area.
        let mut client_scale = dpi_scale;
        for _ in 0..2 {
            let mut frame = RECT {
                left: 0,
                top: 0,
                right: (780.0 * client_scale).round() as i32,
                bottom: (620.0 * client_scale).round() as i32,
            };
            AdjustWindowRectExForDpi(&mut frame, style, 0, WS_EX_CONTROLPARENT, dpi.max(96));
            let available_width = (work_width - 16).max(1);
            let available_height = (work_height - 16).max(1);
            let factor = (available_width as f64 / (frame.right - frame.left).max(1) as f64)
                .min(available_height as f64 / (frame.bottom - frame.top).max(1) as f64)
                .min(1.0);
            client_scale *= factor;
            scale = client_scale;
        }
        let mut frame = RECT {
            left: 0,
            top: 0,
            right: (780.0 * scale).round() as i32,
            bottom: (620.0 * scale).round() as i32,
        };
        AdjustWindowRectExForDpi(&mut frame, style, 0, WS_EX_CONTROLPARENT, dpi.max(96));
        let width = (frame.right - frame.left).min(work_width).max(1);
        let height = (frame.bottom - frame.top).min(work_height).max(1);
        let left = work.left + (work_width - width) / 2;
        let top = work.top + (work_height - height) / 2;
        let result = (
            scale,
            RECT {
                left,
                top,
                right: left + width,
                bottom: top + height,
            },
        );
        if !previous_dpi.is_null() {
            SetThreadDpiAwarenessContext(previous_dpi);
        }
        result
    }

    pub(super) unsafe fn paint_control_center(&self) {
        let mut ps = zeroed();
        let dc = BeginPaint(self.settings_hwnd, &mut ps);
        self.paint_control_center_dc(dc);
        EndPaint(self.settings_hwnd, &ps);
    }

    pub(super) unsafe fn paint_control_center_dc(&self, dc: HDC) {
        let mut client = zeroed();
        GetClientRect(self.settings_hwnd, &mut client);
        FillRect(dc, &client, self.background);
        let sidebar = RECT {
            right: self.center.px(208),
            ..client
        };
        FillRect(dc, &sidebar, self.field);
        let mut rule = RECT {
            left: self.center.px(207),
            right: self.center.px(208),
            top: 0,
            bottom: client.bottom,
        };
        FillRect(dc, &rule, self.selected);
        let footer_y = client.bottom - self.center.px(66);
        if footer_y > self.center.px(120) {
            rule = RECT {
                left: self.center.px(228),
                right: client.right - self.center.px(24),
                top: footer_y,
                bottom: footer_y + self.center.px(1),
            };
            FillRect(dc, &rule, self.selected);
        }
    }

    pub(super) unsafe fn draw_settings_navigation(&self, item: &DRAWITEMSTRUCT) {
        let Some((name, _)) = SECTIONS.get(item.itemID as usize) else {
            return;
        };
        let selected = item.itemState & ODS_SELECTED != 0;
        FillRect(item.hDC, &item.rcItem, self.field);
        let mut tile = item.rcItem;
        InflateRect(&mut tile, -self.center.px(4), -self.center.px(3));
        if selected {
            FillRect(item.hDC, &tile, self.selected);
        }
        let old = SelectObject(item.hDC, self.center.fonts[0]);
        SetTextColor(item.hDC, self.text_color());
        SetBkMode(item.hDC, TRANSPARENT as i32);
        tile.left += self.center.px(12);
        DrawTextW(
            item.hDC,
            wide(name).as_ptr(),
            -1,
            &mut tile,
            DT_SINGLELINE | DT_VCENTER,
        );
        SelectObject(item.hDC, old);
        if item.itemState & ODS_FOCUS != 0 {
            DrawFocusRect(item.hDC, &item.rcItem);
        }
    }
}

unsafe extern "system" fn settings_checkbox_proc(
    hwnd: HWND,
    msg: u32,
    w: usize,
    l: isize,
    subclass: usize,
    dark: usize,
) -> isize {
    if msg == WM_ERASEBKGND {
        return 1;
    }
    if msg == WM_PAINT || msg == WM_PRINTCLIENT {
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = if msg == WM_PRINTCLIENT {
            w as HDC
        } else {
            BeginPaint(hwnd, &mut ps)
        };
        let mut client: RECT = zeroed();
        GetClientRect(hwnd, &mut client);

        let background = if dark != 0 {
            rgb(29, 30, 35)
        } else {
            rgb(247, 248, 250)
        };
        let foreground = if dark != 0 {
            rgb(239, 241, 247)
        } else {
            rgb(28, 31, 38)
        };
        let secondary = if dark != 0 {
            rgb(153, 159, 173)
        } else {
            rgb(115, 121, 133)
        };
        let accent = if dark != 0 {
            rgb(112, 145, 192)
        } else {
            rgb(65, 105, 157)
        };
        let disabled = IsWindowEnabled(hwnd) == 0;
        let fill = CreateSolidBrush(background);
        FillRect(dc, &client, fill);
        DeleteObject(fill);

        let dpi = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
        let side = (16.0 * dpi).round() as i32;
        let x = (1.0 * dpi).round() as i32;
        let y = ((client.bottom - side) / 2).max(0);
        let box_rect = RECT {
            left: x,
            top: y,
            right: x + side,
            bottom: y + side,
        };
        let checked = SendMessageW(hwnd, BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
        let box_fill = CreateSolidBrush(if checked {
            accent
        } else if dark != 0 {
            rgb(38, 39, 46)
        } else {
            rgb(255, 255, 255)
        });
        FillRect(dc, &box_rect, box_fill);
        DeleteObject(box_fill);
        let border = CreatePen(
            PS_SOLID,
            (1.0 * dpi).round().max(1.0) as i32,
            if checked {
                accent
            } else if disabled {
                secondary
            } else if dark != 0 {
                rgb(108, 115, 130)
            } else {
                rgb(145, 153, 166)
            },
        );
        let old_pen = SelectObject(dc, border);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH as i32));
        Rectangle(
            dc,
            box_rect.left,
            box_rect.top,
            box_rect.right,
            box_rect.bottom,
        );
        SelectObject(dc, old_brush);
        SelectObject(dc, old_pen);
        DeleteObject(border);

        if checked {
            let check_pen = CreatePen(
                PS_SOLID,
                (2.0 * dpi).round().max(1.0) as i32,
                if dark != 0 {
                    rgb(247, 248, 250)
                } else {
                    rgb(255, 255, 255)
                },
            );
            let old = SelectObject(dc, check_pen);
            let mut point = POINT {
                x: box_rect.left + (4.0 * dpi).round() as i32,
                y: box_rect.top + (8.0 * dpi).round() as i32,
            };
            MoveToEx(dc, point.x, point.y, null_mut());
            point.x = box_rect.left + (7.0 * dpi).round() as i32;
            point.y = box_rect.top + (11.0 * dpi).round() as i32;
            LineTo(dc, point.x, point.y);
            point.x = box_rect.left + (13.0 * dpi).round() as i32;
            point.y = box_rect.top + (5.0 * dpi).round() as i32;
            LineTo(dc, point.x, point.y);
            SelectObject(dc, old);
            DeleteObject(check_pen);
        }

        let length = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut text = vec![0u16; length + 1];
        if length > 0 {
            GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        }
        let old_font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
        let old_font = if old_font != 0 {
            SelectObject(dc, old_font as HGDIOBJ)
        } else {
            null_mut()
        };
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, if disabled { secondary } else { foreground });
        let mut text_rect = client;
        text_rect.left = box_rect.right + (9.0 * dpi).round() as i32;
        DrawTextW(
            dc,
            text.as_ptr(),
            length as i32,
            &mut text_rect,
            DT_SINGLELINE | DT_VCENTER | DT_END_ELLIPSIS | DT_NOPREFIX,
        );
        if old_font != null_mut() {
            SelectObject(dc, old_font);
        }

        if GetFocus() == hwnd {
            let mut focus = client;
            InflateRect(
                &mut focus,
                -(2.0 * dpi).round() as i32,
                -(2.0 * dpi).round() as i32,
            );
            DrawFocusRect(dc, &focus);
        }
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
        }
        return 0;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(settings_checkbox_proc), subclass);
    }
    DefSubclassProc(hwnd, msg, w, l)
}

unsafe extern "system" fn settings_static_proc(
    hwnd: HWND,
    msg: u32,
    w: usize,
    l: isize,
    subclass: usize,
    ref_data: usize,
) -> isize {
    if msg == WM_ERASEBKGND {
        return 1;
    }
    if msg == WM_PAINT || msg == WM_PRINTCLIENT {
        let data = &*(ref_data as *const StaticPaintData);
        let mut ps: PAINTSTRUCT = zeroed();
        let dc = if msg == WM_PRINTCLIENT {
            w as HDC
        } else {
            BeginPaint(hwnd, &mut ps)
        };
        let dark = data.dark.get();
        let background = if data.sidebar {
            if dark {
                rgb(38, 39, 46)
            } else {
                rgb(255, 255, 255)
            }
        } else if dark {
            rgb(29, 30, 35)
        } else {
            rgb(247, 248, 250)
        };
        let foreground = if data.notice && data.error.get() {
            if dark {
                rgb(255, 164, 157)
            } else {
                rgb(170, 40, 35)
            }
        } else if data.muted {
            if dark {
                rgb(153, 159, 173)
            } else {
                rgb(115, 121, 133)
            }
        } else if dark {
            rgb(239, 241, 247)
        } else {
            rgb(28, 31, 38)
        };
        let fill = CreateSolidBrush(background);
        let mut client: RECT = zeroed();
        GetClientRect(hwnd, &mut client);
        FillRect(dc, &client, fill);
        DeleteObject(fill);

        let length = GetWindowTextLengthW(hwnd).max(0) as usize;
        let mut text = vec![0u16; length + 1];
        if length > 0 {
            GetWindowTextW(hwnd, text.as_mut_ptr(), text.len() as i32);
        }
        let font = SendMessageW(hwnd, WM_GETFONT, 0, 0);
        let old_font = if font != 0 {
            SelectObject(dc, font as HGDIOBJ)
        } else {
            null_mut()
        };
        SetBkMode(dc, TRANSPARENT as i32);
        SetTextColor(dc, foreground);
        DrawTextW(
            dc,
            text.as_ptr(),
            length as i32,
            &mut client,
            DT_LEFT | DT_TOP | DT_WORDBREAK | DT_NOPREFIX,
        );
        if old_font != null_mut() {
            SelectObject(dc, old_font);
        }
        if msg == WM_PAINT {
            EndPaint(hwnd, &ps);
        }
        return 0;
    }
    if msg == WM_NCDESTROY {
        RemoveWindowSubclass(hwnd, Some(settings_static_proc), subclass);
    }
    DefSubclassProc(hwnd, msg, w, l)
}

/// Run before IsDialogMessage so shortcuts work in every native child control.
pub(super) unsafe fn translate_key(msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN {
        return false;
    }
    let parent = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|app| app.settings_hwnd)
            .unwrap_or(null_mut())
    });
    if parent.is_null() || (msg.hwnd != parent && IsChild(parent, msg.hwnd) == 0) {
        return false;
    }
    let ctrl = GetKeyState(VK_CONTROL as i32) < 0;
    if ctrl && GetKeyState(VK_MENU as i32) >= 0 {
        if msg.wParam == b'S' as usize {
            SendMessageW(parent, WM_COMMAND, SAVE, 0);
            return true;
        }
        if (b'1' as usize..=b'6' as usize).contains(&msg.wParam) {
            APP.with(|cell| {
                if let Some(app) = cell.borrow_mut().as_mut() {
                    app.select_settings_section(msg.wParam - b'1' as usize);
                    SetFocus(GetDlgItem(parent, NAV as i32));
                }
            });
            return true;
        }
    }
    if msg.wParam == VK_ESCAPE as usize {
        // A dropped combo owns Esc: close its list, not the Settings window.
        if SendMessageW(msg.hwnd, CB_GETDROPPEDSTATE, 0, 0) != 0 {
            return false;
        }
        SendMessageW(parent, WM_CLOSE, 0, 0);
        return true;
    }
    false
}
