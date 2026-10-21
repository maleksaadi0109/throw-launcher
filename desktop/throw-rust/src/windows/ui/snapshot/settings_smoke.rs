//! Runs on Windows only, inside the isolated --smoke-test profile.
use super::*;

pub(super) unsafe fn verify_and_capture(root: &Path, output: &Path) -> Result<(), String> {
    let parent = APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.show_settings();
        // The bookmark/provider smoke check visits Search first. Settings
        // correctly remembers that section; this test explicitly starts General.
        app.select_settings_section(0);
        app.settings_hwnd
    });
    if parent.is_null() {
        return Err("Settings failed to open.".into());
    }
    settle();
    let nav = GetDlgItem(parent, settings::NAV as i32);
    let save = GetDlgItem(parent, SAVE as i32);
    if SendMessageW(nav, LB_GETCOUNT, 0, 0) != 6 {
        return Err("Settings sidebar is incomplete.".into());
    }
    // Feed Tab through the same dialog manager as the live message loop.
    let mut tab = MSG {
        hwnd: nav,
        message: WM_KEYDOWN,
        wParam: VK_TAB as usize,
        ..zeroed()
    };
    SetFocus(nav);
    IsDialogMessageW(parent, &mut tab);
    if GetFocus() != GetDlgItem(parent, STARTUP as i32) {
        return Err("Settings Tab navigation did not reach General's first control.".into());
    }
    let mut keyboard = [0u8; 256];
    if GetKeyboardState(keyboard.as_mut_ptr()) == 0 {
        return Err("Cannot read keyboard state for Settings checks.".into());
    }
    let mut shifted = keyboard;
    shifted[VK_SHIFT as usize] = 0x80;
    SetKeyboardState(shifted.as_ptr());
    tab.hwnd = GetFocus();
    IsDialogMessageW(parent, &mut tab);
    SetKeyboardState(keyboard.as_ptr());
    if GetFocus() != nav {
        return Err("Settings Shift+Tab did not return to the sidebar.".into());
    }
    // Sidebar Up/Down must select and show a different page, with no draft reset.
    SetFocus(nav);
    SendMessageW(nav, WM_KEYDOWN, VK_DOWN as usize, 0);
    SendMessageW(nav, WM_KEYUP, VK_DOWN as usize, 0);
    settle();
    if IsWindowVisible(GetDlgItem(parent, THEME as i32)) == 0
        || IsWindowVisible(GetDlgItem(parent, STARTUP as i32)) != 0
    {
        return Err("Native sidebar arrow navigation failed.".into());
    }
    let limit = GetDlgItem(parent, LIMIT as i32);
    SendMessageW(limit, CB_SETCURSEL, 2, 0);
    verify_discard_cancel(parent)?;
    // Theme commands must update Appearance without losing this Search draft.
    for (query, expected) in [("Light theme", Theme::Light), ("Dark theme", Theme::Ink)] {
        let edit = APP.with(|cell| cell.borrow().as_ref().unwrap().edit);
        SetWindowTextW(edit, wide(query).as_ptr());
        settle();
        APP.with(|cell| cell.borrow_mut().as_mut().unwrap().activate());
        settle();
        let okay = APP.with(|cell| {
            let slot = cell.borrow();
            let app = slot.as_ref().unwrap();
            app.settings.theme == expected
                && SendMessageW(GetDlgItem(parent, THEME as i32), CB_GETCURSEL, 0, 0)
                    == expected as isize
                && SendMessageW(limit, CB_GETCURSEL, 0, 0) == 2
                && app.settings_dirty()
        });
        if !okay {
            return Err(
                "Appearance command lost the Settings draft or failed to synchronize.".into(),
            );
        }
    }
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.save_settings()?;
        if app.settings.max_results != 100
            || app.settings_dirty()
            || Settings::load(&root.join("settings.json"))? != app.settings
        {
            return Err("Settings save/reload or clean-state tracking failed.".into());
        }
        // Fail the write without changing startup registration or global hotkeys.
        let original = app.settings.clone();
        SendMessageW(limit, CB_SETCURSEL, 0, 0);
        let blocked = root.join("blocked-profile");
        std::fs::write(&blocked, b"not a directory").map_err(|e| e.to_string())?;
        app.root = blocked;
        let result = app.save_settings();
        app.root = root.to_path_buf();
        if result.is_ok()
            || app.settings != original
            || !app.settings_dirty()
            || SendMessageW(limit, CB_GETCURSEL, 0, 0) != 0
        {
            return Err("A failed save lost the draft or changed live settings.".into());
        }
        SendMessageW(limit, CB_SETCURSEL, 2, 0);
        Ok::<_, String>(())
    })?;
    // Ctrl+number and Ctrl+S use the actual live pre-translation path.
    let mut ctrl = keyboard;
    ctrl[VK_CONTROL as usize] = 0x80;
    SetKeyboardState(ctrl.as_ptr());
    let section_key = MSG {
        hwnd: nav,
        message: WM_KEYDOWN,
        wParam: b'5' as usize,
        ..zeroed()
    };
    let section_handled = settings::translate_key(&section_key);
    let save_key = MSG {
        hwnd: nav,
        message: WM_KEYDOWN,
        wParam: b'S' as usize,
        ..zeroed()
    };
    let save_handled = settings::translate_key(&save_key);
    SetKeyboardState(keyboard.as_ptr());
    if !section_handled || !save_handled || IsWindowVisible(GetDlgItem(parent, HISTORY as i32)) == 0
    {
        return Err("Settings Ctrl+number or Ctrl+S failed.".into());
    }
    // Every page has reachable visible controls and persistent Save/Cancel.
    for theme in [Theme::Ink, Theme::Light] {
        for section in 0..6 {
            APP.with(|cell| {
                let mut slot = cell.borrow_mut();
                let app = slot.as_mut().unwrap();
                app.settings.theme = theme;
                app.rebuild_brushes();
                app.sync_settings_appearance();
                app.select_settings_section(section);
            });
            settle();
            SetForegroundWindow(parent);
            RedrawWindow(
                parent,
                null(),
                null_mut(),
                RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
            );
            let first = GetNextDlgTabItem(parent, nav, 0);
            if first.is_null()
                || first == nav
                || IsWindowVisible(first) == 0
                || IsWindowVisible(save) == 0
            {
                return Err(format!("Settings section {section} has no keyboard path."));
            }
            let name = if theme == Theme::Ink { "ink" } else { "paper" };
            capture(
                parent,
                &output.join(format!("throw-settings-{name}-{section}.bmp")),
            )?;
        }
    }
    // Exercise the WM_DPICHANGED layout at real device-pixel sizes. This is not
    // a claim that a CI VM has two physical monitors with different scale factors.
    let mut original: RECT = zeroed();
    GetWindowRect(parent, &mut original);
    let original_dpi = windows_sys::Win32::UI::HiDpi::GetDpiForWindow(parent).max(96);
    for dpi in [96, 144, 192] {
        let scale = dpi as f64 / 96.0;
        let rect = RECT {
            left: 0,
            top: 0,
            right: (800.0 * scale) as i32,
            bottom: (540.0 * scale) as i32,
        };
        SendMessageW(
            parent,
            WM_DPICHANGED,
            dpi as usize | ((dpi as usize) << 16),
            (&rect as *const RECT) as isize,
        );
        APP.with(|cell| {
            cell.borrow_mut()
                .as_mut()
                .unwrap()
                .select_settings_section(1)
        });
        settle();
        let mut client: RECT = zeroed();
        GetClientRect(parent, &mut client);
        for control in [nav, GetDlgItem(parent, THEME as i32), save] {
            let mut bounds: RECT = zeroed();
            GetWindowRect(control, &mut bounds);
            MapWindowPoints(null_mut(), parent, (&mut bounds as *mut RECT).cast(), 2);
            if bounds.left < 0
                || bounds.top < 0
                || bounds.right > client.right
                || bounds.bottom > client.bottom
            {
                return Err(format!(
                    "Settings control {} clipped at {dpi} DPI: {},{},{},{} in {},{}.",
                    GetDlgCtrlID(control),
                    bounds.left,
                    bounds.top,
                    bounds.right,
                    bounds.bottom,
                    client.right,
                    client.bottom
                ));
            }
        }
        RedrawWindow(
            parent,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
        );
        capture(
            parent,
            &output.join(format!("throw-settings-dpi-{dpi}.bmp")),
        )?;
    }
    SendMessageW(
        parent,
        WM_DPICHANGED,
        original_dpi as usize | ((original_dpi as usize) << 16),
        (&original as *const RECT) as isize,
    );
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.close_control_center();
        app.settings.theme = Theme::Ink;
        app.rebuild_brushes();
    });
    verify_import(root)?;
    Ok(())
}

unsafe fn verify_import(root: &Path) -> Result<(), String> {
    let source = root.join("source-preferences.json");
    let bytes = br#"{"HideOnStartup":true,"HideWhenDeactivated":false,"Plugins":[{"command":"not imported"}]}"#;
    std::fs::write(&source, bytes).map_err(|e| e.to_string())?;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.show_settings();
        // An invalid file must leave the live profile and its controls alone.
        let before = app.settings.clone();
        let invalid = root.join("invalid-preferences.json");
        std::fs::write(&invalid, b"[]").map_err(|e| e.to_string())?;
        if app.import_settings_file(&invalid).is_ok() || app.settings != before {
            return Err("Invalid import changed live settings.".into());
        }
        Ok::<_, String>(())
    })?;
    let before = Settings::load(&root.join("settings.json"))?;
    let decline = answer_dialog("Import preferences", IDNO);
    let declined = APP.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .unwrap()
            .import_settings_file(&source)
    });
    if !decline.join().unwrap_or(false) {
        return Err("Import cancellation was not exercised.".into());
    }
    declined?;
    if Settings::load(&root.join("settings.json"))? != before {
        return Err("Cancelled import wrote settings.".into());
    }
    let accept = answer_dialog("Import preferences", IDYES);
    let imported = APP.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .unwrap()
            .import_settings_file(&source)
    });
    if !accept.join().unwrap_or(false) {
        return Err("Import confirmation was not exercised.".into());
    }
    imported?;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        if !app.settings.lite
            || !app.settings.hide_on_startup
            || app.settings.hide_on_focus_loss
            || Settings::load(&root.join("settings.json"))? != app.settings
            || std::fs::read(&source).map_err(|e| e.to_string())? != bytes
            || !std::fs::read_dir(root)
                .map_err(|e| e.to_string())?
                .flatten()
                .any(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with("settings-before-import-")
                })
        {
            return Err(
                "Import failed to preserve the source, back up or persist supported preferences."
                    .into(),
            );
        }
        app.close_control_center();
        app.show_settings();
        if app.settings_dirty()
            || SendMessageW(
                GetDlgItem(app.settings_hwnd, HIDE_START as i32),
                BM_GETCHECK,
                0,
                0,
            ) != BST_CHECKED as isize
        {
            return Err("Reopening Settings did not reflect imported preferences.".into());
        }
        Ok::<_, String>(())
    })?;
    let parent = APP.with(|cell| cell.borrow().as_ref().unwrap().settings_hwnd);
    let escape = MSG {
        hwnd: parent,
        message: WM_KEYDOWN,
        wParam: VK_ESCAPE as usize,
        ..zeroed()
    };
    if !settings::translate_key(&escape)
        || APP.with(|cell| !cell.borrow().as_ref().unwrap().settings_hwnd.is_null())
    {
        return Err("Escape did not close clean Settings.".into());
    }
    Ok(())
}

unsafe fn verify_discard_cancel(parent: HWND) -> Result<(), String> {
    let decline = answer_dialog("Unsaved changes — Throw", IDNO);
    SendMessageW(parent, WM_COMMAND, settings::CANCEL, 0);
    if !decline.join().unwrap_or(false) || IsWindow(parent) == 0 {
        return Err("Cancel discarded edits without confirmation.".into());
    }
    if SendMessageW(GetDlgItem(parent, LIMIT as i32), CB_GETCURSEL, 0, 0) != 2 {
        return Err("Declining discard lost the Settings draft.".into());
    }
    Ok(())
}

fn answer_dialog(title: &'static str, answer: i32) -> std::thread::JoinHandle<bool> {
    std::thread::spawn(move || unsafe {
        for _ in 0..500 {
            let dialog = FindWindowW(wide("#32770").as_ptr(), wide(title).as_ptr());
            if !dialog.is_null() {
                SendMessageW(dialog, WM_COMMAND, answer as usize, 0);
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    })
}
