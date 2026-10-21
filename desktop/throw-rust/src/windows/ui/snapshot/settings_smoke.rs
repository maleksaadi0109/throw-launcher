//! Runs on Windows only, inside the isolated --smoke-test profile.
use super::*;

pub(super) unsafe fn verify_and_capture(root: &Path, output: &Path) -> Result<(), String> {
    let parent = APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.show_settings();
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
                return Err(format!("Settings control clipped at {dpi} DPI."));
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
    Ok(())
}
