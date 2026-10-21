//! Native Windows visual smoke evidence. Only invoked with --smoke-test.
use super::*;
mod settings_smoke;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) unsafe fn verify_and_capture(root: &Path) -> Result<(), String> {
    verify_edit_shortcuts()?;
    verify_folders_and_scroll(root)?;
    verify_command_palette()?;
    super::web_smoke::verify(root)?;
    let menu = actions::build_menu();
    if menu.is_null() || GetMenuItemCount(menu) != 7 {
        return Err("Result action menu is incomplete.".into());
    }
    DestroyMenu(menu);
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let mut msg: MSG = zeroed();
        while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let ready = APP.with(|cell| {
            let slot = cell.borrow();
            let app = slot.as_ref().unwrap();
            app.entries.iter().all(|entry| {
                app.icons
                    .as_ref()
                    .and_then(|icons| icons.get(&entry.path))
                    .is_some()
            }) && app.folders.iter().all(|entry| {
                app.icons
                    .as_ref()
                    .and_then(|icons| icons.get(&entry.path))
                    .is_some()
            })
        });
        if ready {
            break;
        }
        if Instant::now() >= deadline {
            let missing = APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.entries
                    .iter()
                    .filter(|entry| {
                        app.icons
                            .as_ref()
                            .and_then(|icons| icons.get(&entry.path))
                            .is_none()
                    })
                    .map(|entry| format!("{} (exists: {})", entry.name, entry.path.is_file()))
                    .collect::<Vec<_>>()
                    .join(", ")
            });
            return Err(format!(
                "Real Shell icons did not reach the result list: {missing}"
            ));
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    let hwnd = MAIN_WINDOW.with(|window| window.get());
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.settings.theme = Theme::Ink;
        app.rebuild_brushes();
    });
    let output = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Missing executable directory")?
        .to_path_buf();
    settings_smoke::verify_and_capture(root, &output)?;
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    capture(hwnd, &output.join("throw-ink.bmp"))?;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.settings.theme = Theme::Light;
        app.rebuild_brushes();
    });
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    capture(hwnd, &output.join("throw-paper.bmp"))?;
    let edit = APP.with(|cell| cell.borrow().as_ref().unwrap().edit);
    SetWindowTextW(edit, wide(">").as_ptr());
    settle();
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    capture(hwnd, &output.join("throw-commands.bmp"))?;
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    let count = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .icons
            .as_ref()
            .unwrap()
            .len()
    });
    if count > 48 {
        return Err("Lite icon cache exceeded its bound.".into());
    }
    let report = format!("{}\nReal Shell icons loaded for apps and folders.\nThree-row viewport, native scrolling, folder/path search and background-selection stability verified.\nNative Ctrl+A/C/V/X/Z editing verified with focus in search and results. Alt+1 calculator activation verified.\nCommand palette binding, reboot alias and native confirmation cancellation verified. No power/session/delete action was executed; successful system changes require real-device testing.\nSettings: six sections, keyboard focus, draft preservation, save/reload, failed-save recovery, theme-command synchronization and 96/144/192 DPI layout verified. Native captures include all six sections in both themes and Appearance at 150%/200%. Real mixed-monitor dragging still needs device verification.\nApp and folder menus constructed; shortcut target resolution covered by Windows tests. No apps, folders or elevated processes launched.\n{count} icons cached. Lite maximum 48; Standard maximum 96.\nThis CI sample is not a real-world idle memory benchmark.", memory_report()?);
    let report = format!("{report}\nNative bookmark opt-in/disable, fixture discovery, safe URL binding, error reporting and provider create/edit/disable/delete persistence verified. Invalid and failed saves preserved previous settings. No personal browser profiles read or browsers launched.");
    std::fs::write(root.join("smoke-result.txt"), report).map_err(|e| e.to_string())?;
    Ok(())
}

unsafe fn settle() {
    let mut message: MSG = zeroed();
    while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
        TranslateMessage(&message);
        DispatchMessageW(&message);
    }
}

unsafe fn verify_command_palette() -> Result<(), String> {
    use throw_launcher::commands::SystemCommand;
    let (edit, list, hwnd) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (app.edit, app.list, app.hwnd)
    });
    SetWindowTextW(edit, wide(">").as_ptr());
    settle();
    if SendMessageW(list, LB_GETCOUNT, 0, 0) < 30 {
        return Err("Command palette did not bind the complete catalog.".into());
    }
    SetWindowTextW(edit, wide("reboot").as_ptr());
    let restart = APP.with(|cell| {
        matches!(
            cell.borrow()
                .as_ref()
                .unwrap()
                .rows
                .first()
                .map(|row| &row.action),
            Some(Action::System(SystemCommand::Restart))
        )
    });
    if !restart {
        return Err("Reboot alias did not bind the restart action.".into());
    }
    ShowWindow(hwnd, SW_SHOW);
    // Exercise the real Enter -> confirmation -> No path. Never click Yes or
    // dispatch a power operation, even if the test fails.
    let dismiss = std::thread::spawn(|| {
        for _ in 0..500 {
            let dialog = FindWindowW(
                wide("#32770").as_ptr(),
                wide("Confirm system action — Throw").as_ptr(),
            );
            if !dialog.is_null() {
                SendMessageW(dialog, WM_COMMAND, IDNO as usize, 0);
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    });
    SendMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
    if !dismiss.join().unwrap_or(false) || IsWindowVisible(hwnd) == 0 {
        return Err("Restart confirmation cancellation failed.".into());
    }
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    Ok(())
}

unsafe fn verify_folders_and_scroll(root: &Path) -> Result<(), String> {
    let (edit, list, hwnd) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (app.edit, app.list, app.hwnd)
    });
    SetWindowTextW(edit, wide("Documents").as_ptr());
    let is_folder = APP.with(|cell| {
        matches!(cell.borrow().as_ref().unwrap().rows.first().map(|r| &r.action), Some(Action::Folder(path)) if path.is_dir())
    });
    if !is_folder {
        return Err("Folder name did not produce an openable folder result.".into());
    }
    SetWindowTextW(edit, wide(root.join("Projects")).as_ptr());
    if SendMessageW(list, LB_GETCOUNT, 0, 0) < 1 {
        return Err("Absolute local folder browsing failed.".into());
    }
    // A late index update must not silently change what Enter will launch.
    SetWindowTextW(edit, wide("throw").as_ptr());
    // Commands now also match "throw": explicitly select the app to keep this
    // regression checking app identity across an asynchronously arriving folder.
    let app_row = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .rows
            .iter()
            .position(|row| matches!(row.action, Action::App(_)))
    });
    let Some(app_row) = app_row else {
        return Err("Throw app fixture missing.".into());
    };
    SendMessageW(list, LB_SETCURSEL, app_row, 0);
    let path = root.join("Throw");
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    let before = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
    let (sender, receiver) = mpsc::channel();
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        let mut folders = app.folders.clone();
        folders.push(Folder::new(path).unwrap());
        sender
            .send(Ok(IndexResult {
                bookmarks: bookmarks::Scan::default(),
                apps: Scan {
                    entries: app.entries.clone(),
                    unreadable: 0,
                    truncated: false,
                },
                folders: folders::Scan {
                    entries: folders,
                    limited: false,
                    unreadable: 0,
                },
            }))
            .unwrap();
        app.worker = Some(receiver);
    });
    SendMessageW(hwnd, INDEX_READY, 0, 0);
    let after = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
    if before != after {
        return Err("Background folder search changed the selected action.".into());
    }
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    let mut rect: RECT = zeroed();
    GetClientRect(list, &mut rect);
    let row_height = SendMessageW(list, LB_GETITEMHEIGHT, 0, 0) as i32;
    if rect.bottom != row_height * 3 || SendMessageW(list, LB_GETCOUNT, 0, 0) <= 3 {
        return Err(format!("Expected three visible rows and more scrollable results; height {}, row height {row_height}.", rect.bottom));
    }
    if GetWindowLongPtrW(list, GWL_STYLE) as u32 & WS_VSCROLL != 0 {
        return Err("Result scrollbar chrome is still enabled.".into());
    }
    // Exercise the actual wheel route, including two high-resolution half ticks.
    RESULT_WHEEL_REMAINDER.with(|value| value.set(0));
    SendMessageW(list, WM_MOUSEWHEEL, ((-60i16 as u16 as usize) << 16), 0);
    if SendMessageW(list, LB_GETTOPINDEX, 0, 0) != 0 {
        return Err("Partial wheel delta was not accumulated.".into());
    }
    SendMessageW(list, WM_MOUSEWHEEL, ((-60i16 as u16 as usize) << 16), 0);
    if SendMessageW(list, LB_GETTOPINDEX, 0, 0) <= 0 {
        return Err("Hidden-scrollbar wheel route did not reveal more results.".into());
    }
    SendMessageW(list, LB_SETTOPINDEX, 0, 0);
    SendMessageW(list, WM_KEYDOWN, VK_END as usize, 0);
    SendMessageW(list, WM_KEYUP, VK_END as usize, 0);
    if SendMessageW(list, LB_GETTOPINDEX, 0, 0) <= 0 {
        return Err("Keyboard navigation no longer scrolls hidden-bar results.".into());
    }
    SendMessageW(list, LB_SETTOPINDEX, 0, 0);
    SendMessageW(list, LB_SETCURSEL, 0, 0);
    let menu = actions::build_menu_for(true);
    if menu.is_null() || GetMenuItemCount(menu) != 4 {
        return Err("Folder context menu is incomplete.".into());
    }
    DestroyMenu(menu);
    Ok(())
}

unsafe fn verify_edit_shortcuts() -> Result<(), String> {
    let (edit, list) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (app.edit, app.list)
    });
    let mut keyboard = [0u8; 256];
    if GetKeyboardState(keyboard.as_mut_ptr()) == 0 {
        return Err("Cannot read keyboard state in editing test.".into());
    }
    let result = (|| {
        let mut pressed = keyboard;
        pressed[VK_CONTROL as usize] = 0x80;
        pressed[VK_MENU as usize] = 0;
        if SetKeyboardState(pressed.as_ptr()) == 0 {
            return Err("Cannot set keyboard state in editing test.".into());
        }
        let key = |hwnd, value| {
            SendMessageW(hwnd, WM_KEYDOWN, value, 0);
        };
        let value = || {
            let mut text = [0u16; 513];
            let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32);
            String::from_utf16_lossy(&text[..length.max(0) as usize])
        };
        SetWindowTextW(edit, wide("clipboard test").as_ptr());
        // Also exercise routing from the result list, not just the edit field.
        SetFocus(list);
        key(list, b'A' as usize);
        let mut start = 0u32;
        let mut end = 0u32;
        SendMessageW(
            edit,
            EM_GETSEL,
            (&mut start as *mut u32) as usize,
            (&mut end as *mut u32) as isize,
        );
        if start != 0 || end != 14 {
            return Err("Ctrl+A failed to select the complete query.".into());
        }
        key(edit, b'C' as usize);
        SetWindowTextW(edit, wide("replace this").as_ptr());
        key(edit, b'A' as usize);
        key(edit, b'V' as usize);
        if value() != "clipboard test" {
            return Err("Ctrl+C / Ctrl+V roundtrip failed.".into());
        }
        key(edit, b'A' as usize);
        key(edit, b'X' as usize);
        if !value().is_empty() {
            return Err("Ctrl+X did not cut the selected query.".into());
        }
        // Let deferred geometry settle before undo, as it would between real
        // keystrokes. Editing must survive a results-height change.
        let mut message: MSG = zeroed();
        while PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) != 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        key(edit, b'Z' as usize);
        if value() != "clipboard test" {
            return Err(format!(
                "Ctrl+Z did not restore the cut query (got {:?}, can undo: {}).",
                value(),
                SendMessageW(edit, EM_CANUNDO, 0, 0)
            ));
        }
        SetWindowTextW(edit, wide("= 6 * 7").as_ptr());
        let mut alt = keyboard;
        alt[VK_MENU as usize] = 0x80;
        alt[VK_CONTROL as usize] = 0;
        SetKeyboardState(alt.as_ptr());
        SendMessageW(edit, WM_SYSKEYDOWN, b'1' as usize, 0);
        SendMessageW(edit, WM_SYSCHAR, b'1' as usize, 0);
        SetKeyboardState(pressed.as_ptr());
        SetWindowTextW(edit, wide("").as_ptr());
        key(edit, b'V' as usize);
        if value() != "42" {
            return Err("Alt+1 did not activate the selected calculator result.".into());
        }
        ShowWindow(GetParent(edit), SW_SHOW);
        Ok(())
    })();
    SetKeyboardState(keyboard.as_ptr());
    SetWindowTextW(edit, wide("").as_ptr());
    result
}

pub(super) unsafe fn capture(hwnd: HWND, path: &Path) -> Result<(), String> {
    let mut client: RECT = zeroed();
    GetClientRect(hwnd, &mut client);
    let width = client.right;
    let height = client.bottom;
    if width <= 0 || height <= 0 || width > 4000 || height > 4000 {
        return Err("Invalid native screenshot dimensions.".into());
    }
    let dc = GetDC(hwnd);
    if dc.is_null() {
        return Err("Cannot capture native client DC.".into());
    }
    let memory = CreateCompatibleDC(dc);
    let bitmap = CreateCompatibleBitmap(dc, width, height);
    if memory.is_null() || bitmap.is_null() {
        if !memory.is_null() {
            DeleteDC(memory);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        ReleaseDC(hwnd, dc);
        return Err("Cannot allocate native screenshot.".into());
    }
    let previous = SelectObject(memory, bitmap);
    let settings = APP.with(|cell| cell.borrow().as_ref().unwrap().settings_hwnd == hwnd);
    // Print native Settings children even where a synthetic high-DPI window
    // extends beyond the CI desktop. Launcher captures still use its screen DC.
    let copied = if settings {
        windows_sys::Win32::Storage::Xps::PrintWindow(
            hwnd,
            memory,
            windows_sys::Win32::Storage::Xps::PW_CLIENTONLY | PW_RENDERFULLCONTENT,
        )
    } else {
        BitBlt(memory, 0, 0, width, height, dc, 0, 0, SRCCOPY)
    };
    SelectObject(memory, previous);
    let mut info: BITMAPINFO = zeroed();
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width;
    info.bmiHeader.biHeight = height;
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let lines = GetDIBits(
        dc,
        bitmap,
        0,
        height as u32,
        pixels.as_mut_ptr().cast(),
        &mut info,
        DIB_RGB_COLORS,
    );
    DeleteObject(bitmap);
    DeleteDC(memory);
    ReleaseDC(hwnd, dc);
    if copied == 0 || lines != height {
        return Err("Native screenshot capture failed.".into());
    }
    let mut bytes = Vec::with_capacity(54 + pixels.len());
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
    bytes.extend_from_slice(&[0; 4]);
    bytes.extend_from_slice(&54u32.to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&width.to_le_bytes());
    bytes.extend_from_slice(&height.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&32u16.to_le_bytes());
    bytes.extend_from_slice(&[0; 24]);
    bytes.extend_from_slice(&pixels);
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}
