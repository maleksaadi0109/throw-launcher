//! Native Windows visual smoke evidence. Only invoked with --smoke-test.
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) unsafe fn verify_and_capture(root: &Path) -> Result<(), String> {
    verify_edit_shortcuts()?;
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
    let output = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("Missing executable directory")?
        .to_path_buf();
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
    let report = format!("{}\nReal Shell icons loaded for all three smoke-test apps.\nNative Ctrl+A/C/V/X/Z editing verified with focus in search and results. Alt+1 calculator activation verified.\nSeven app context actions constructed; shortcut target resolution covered by Windows tests. No apps or elevated processes launched.\n{count} icons cached. Lite maximum 48; Standard maximum 96.\nThis CI sample is not a real-world idle memory benchmark.", memory_report()?);
    std::fs::write(root.join("smoke-result.txt"), report).map_err(|e| e.to_string())?;
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
        key(edit, b'Z' as usize);
        if value() != "clipboard test" {
            return Err("Ctrl+Z did not restore the cut query.".into());
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

unsafe fn capture(hwnd: HWND, path: &Path) -> Result<(), String> {
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
    let copied = BitBlt(memory, 0, 0, width, height, dc, 0, 0, SRCCOPY);
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
