//! Exercise the real inline panel without launching or elevating applications.
use super::*;

pub(super) unsafe fn verify(output: &Path) -> Result<(), String> {
    let (hwnd, edit, list) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (app.hwnd, app.edit, app.list)
    });
    SetWindowTextW(edit, wide("throw").as_ptr());
    settle();
    let selected = APP
        .with(|cell| {
            cell.borrow()
                .as_ref()
                .unwrap()
                .rows
                .iter()
                .position(|r| matches!(r.action, Action::App(_)))
        })
        .ok_or("No app fixture for action panel.")?;
    SendMessageW(list, LB_SETCURSEL, selected, 0);
    let identity = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
    actions::context_menu(list, -1);
    settle();
    if SendMessageW(list, LB_GETCOUNT, 0, 0) != 7 || GetFocus() != edit {
        return Err("Right click did not open the inline action panel.".into());
    }
    // A real asynchronous refresh must leave the captured action target intact.
    let target = APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .entry
            .path
            .clone()
    });
    SendMessageW(list, LB_SETCURSEL, 1, 0);
    APP.with(|cell| cell.borrow_mut().as_mut().unwrap().refresh());
    if SendMessageW(list, LB_GETCURSEL, 0, 0) != 1 {
        return Err("Asynchronous refresh changed the selected context action.".into());
    }
    if APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .unwrap()
            .context
            .as_ref()
            .unwrap()
            .entry
            .path
            .clone()
    }) != target
    {
        return Err("Refresh changed the action panel target.".into());
    }
    // Capture the five visible full-width rows, not an HMENU.
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.settings.theme = Theme::Ink;
        app.rebuild_brushes();
    });
    RedrawWindow(
        hwnd,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    capture(hwnd, &output.join("throw-inline-actions.bmp"))?;
    // Filtering exposes Copy path; Enter must not launch the app or close UI.
    SetWindowTextW(edit, wide("copy").as_ptr());
    settle();
    if SendMessageW(list, LB_GETCOUNT, 0, 0) != 1 {
        return Err("Action filtering did not keep Copy path.".into());
    }
    SendMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_none())
        || IsWindowVisible(hwnd) == 0
    {
        return Err("Copy path unexpectedly closed the action panel.".into());
    }
    // Use the edit's native paste to verify exactly which target was copied.
    SendMessageW(edit, EM_SETSEL, 0, -1);
    SendMessageW(edit, WM_PASTE, 0, 0);
    let mut pasted = [0u16; 4097];
    let length = GetWindowTextW(edit, pasted.as_mut_ptr(), pasted.len() as i32).max(0) as usize;
    if String::from_utf16_lossy(&pasted[..length]) != target.to_string_lossy() {
        return Err("Copy path used the wrong captured target.".into());
    }
    SendMessageW(edit, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    settle();
    if APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        app.context.is_some() || app.selected_identity() != identity
    }) {
        return Err("Esc did not restore the original search selection.".into());
    }
    // Folder panels omit app-only elevation/target actions.
    SetWindowTextW(edit, wide("Documents").as_ptr());
    settle();
    actions::context_menu(list, -1);
    settle();
    if SendMessageW(list, LB_GETCOUNT, 0, 0) != 4 {
        return Err("Folder action panel contains app-only actions.".into());
    }
    SendMessageW(hwnd, WM_COMMAND, SETTINGS_BUTTON, 0); // Back
    settle();
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_some()) {
        return Err("Back did not leave the action panel.".into());
    }
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    Ok(())
}
