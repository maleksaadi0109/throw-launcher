//! Exercise the real inline panel without launching or elevating applications.
use super::*;

/// Route the real shortcut through the focused edit, restoring modifier state
/// even when a check fails. Include the control character TranslateMessage emits.
unsafe fn open_from_search(edit: HWND) -> Result<(), String> {
    SetFocus(edit);
    with_modifiers(true, false, false, || {
        SendMessageW(edit, WM_KEYDOWN, b'K' as usize, 0);
        SendMessageW(edit, WM_CHAR, 11, 0);
        Ok(())
    })?;
    settle();
    Ok(())
}

unsafe fn query(edit: HWND) -> String {
    let mut text = [0u16; 513];
    let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
    String::from_utf16_lossy(&text[..length])
}

/// Use an eligible app row for modifier exclusions, so a broken exclusion
/// cannot pass merely because the selected action has no panel.
unsafe fn verify_modifier_exclusions(
    hwnd: HWND,
    edit: HWND,
    list: HWND,
    selected: usize,
) -> Result<(), String> {
    let original_query = query(edit);
    let identity = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
    SetFocus(edit);
    SendMessageW(edit, EM_SETSEL, usize::MAX, -1);
    with_modifiers(false, false, false, || {
        SendMessageW(edit, WM_KEYDOWN, b'K' as usize, 0);
        // SendMessage does not call TranslateMessage: deliver its printable
        // character explicitly, just as for the Ctrl+K control character.
        SendMessageW(edit, WM_CHAR, b'k' as usize, 0);
        Ok(())
    })?;
    settle();
    if query(edit) != format!("{original_query}k")
        || APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_some())
        || GetFocus() != edit
        || IsWindowVisible(hwnd) == 0
    {
        return Err("Plain K did not remain normal search typing.".into());
    }
    SetWindowTextW(edit, wide(&original_query).as_ptr());
    settle();
    SendMessageW(list, LB_SETCURSEL, selected, 0);
    for (label, control, shift, alt) in [
        ("Ctrl+Shift+K", true, true, false),
        ("Alt+K", false, false, true),
        ("Alt+Shift+K", false, true, true),
        ("Ctrl+Alt+K", true, false, true),
        ("Ctrl+Alt+Shift+K", true, true, true),
    ] {
        let top = SendMessageW(list, LB_GETTOPINDEX, 0, 0);
        with_modifiers(control, shift, alt, || {
            if alt {
                let flags = 1isize << 29; // Native Alt context bit.
                SendMessageW(edit, WM_SYSKEYDOWN, b'K' as usize, flags);
                SendMessageW(
                    edit,
                    WM_SYSCHAR,
                    if control { 11 } else { b'k' as usize },
                    flags,
                );
                // AltGr can also arrive via the ordinary keydown path.
                if control {
                    SendMessageW(edit, WM_KEYDOWN, b'K' as usize, 0);
                    SendMessageW(edit, WM_CHAR, 11, 0);
                }
            } else {
                SendMessageW(edit, WM_KEYDOWN, b'K' as usize, 0);
                SendMessageW(edit, WM_CHAR, 11, 0);
            }
            Ok(())
        })?;
        settle();
        if query(edit) != original_query
            || APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.context.is_some() || app.selected_identity() != identity
            })
            || SendMessageW(list, LB_GETCURSEL, 0, 0) != selected as isize
            || SendMessageW(list, LB_GETTOPINDEX, 0, 0) != top
            || GetFocus() != edit
            || IsWindowVisible(hwnd) == 0
        {
            return Err(format!("{label} opened a panel or changed the search."));
        }
    }
    Ok(())
}

pub(super) unsafe fn verify(root: &Path, output: &Path) -> Result<(), String> {
    let (hwnd, edit, list) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (app.hwnd, app.edit, app.list)
    });
    // In-memory entries only: no applications or folders are created/launched.
    // A unique query avoids depending on the machine's installed applications.
    let (original_entries, original_max_results) = APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        let entries = (0..12)
            .map(|i| Entry {
                name: format!("Throw Scroll Fixture {i:02}"),
                path: output.join(format!("throw-scroll-fixture-{i:02}.exe")),
            })
            .collect();
        (
            std::mem::replace(&mut app.entries, entries),
            std::mem::replace(&mut app.settings.max_results, 12),
        )
    });
    // Preserve case and surrounding spaces, not just normalized search terms.
    let original_query = "  ThRoW ScRoLl FiXtUrE  ";
    SetWindowTextW(edit, wide(original_query).as_ptr());
    settle();
    if SendMessageW(list, LB_GETCOUNT, 0, 0) != 12 {
        return Err("Scrolled action-panel fixture did not produce twelve results.".into());
    }
    let selected = 5;
    let original_top = 4;
    SendMessageW(list, LB_SETCURSEL, selected, 0);
    verify_modifier_exclusions(hwnd, edit, list, selected)?;
    // Keep the selected row visible but distinct from restore_selection's
    // default top (selected - 2), so losing the saved viewport cannot pass.
    SendMessageW(list, LB_SETTOPINDEX, original_top, 0);
    SetFocus(edit);
    settle();
    let identity = APP
        .with(|cell| cell.borrow().as_ref().unwrap().selected_identity())
        .ok_or("Scrolled action-panel fixture has no selected identity.")?;
    if query(edit) != original_query
        || SendMessageW(list, LB_GETCURSEL, 0, 0) != selected as isize
        || SendMessageW(list, LB_GETTOPINDEX, 0, 0) != original_top as isize
        || GetFocus() != edit
    {
        return Err("Action-panel fixture did not establish a focused, scrolled search.".into());
    }
    open_from_search(edit)?;
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_none())
        || SendMessageW(list, LB_GETCOUNT, 0, 0) != 7
        || GetFocus() != edit
        || !query(edit).is_empty()
    {
        return Err("Ctrl+K from search did not open the app action panel.".into());
    }
    // Repeated Ctrl+K must not replace the saved query/selection with panel state.
    open_from_search(edit)?;
    SendMessageW(edit, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    settle();
    if APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        app.context.is_some() || app.selected_identity().as_ref() != Some(&identity)
    }) || query(edit) != original_query
        || SendMessageW(list, LB_GETTOPINDEX, 0, 0) != original_top as isize
        || GetFocus() != edit
        || IsWindowVisible(hwnd) == 0
    {
        return Err("Esc after Ctrl+K did not restore the original app search.".into());
    }
    // Keep the existing context-menu entry point covered as well.
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
            .target
            .clone()
    });
    SendMessageW(list, LB_SETCURSEL, 1, 0);
    APP.with(|cell| cell.borrow_mut().as_mut().unwrap().refresh());
    if SendMessageW(list, LB_GETCURSEL, 0, 0) != 1 {
        return Err("Asynchronous refresh changed the selected context action.".into());
    }
    if !same_app(
        &APP.with(|cell| {
            cell.borrow()
                .as_ref()
                .unwrap()
                .context
                .as_ref()
                .unwrap()
                .target
                .clone()
        }),
        &target,
    ) {
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
    let actions::Target::App(target) = target else {
        return Err("App panel did not capture an app.".into());
    };
    if String::from_utf16_lossy(&pasted[..length]) != target.path.to_string_lossy() {
        return Err("Copy path used the wrong captured target.".into());
    }
    SendMessageW(edit, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    settle();
    if APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        app.context.is_some() || app.selected_identity().as_ref() != Some(&identity)
    }) {
        return Err("Esc did not restore the original search selection.".into());
    }
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.entries = original_entries;
        app.settings.max_results = original_max_results;
    });
    // Folder panels omit app-only elevation/target actions.
    SetWindowTextW(edit, wide("Documents").as_ptr());
    settle();
    let folder_identity = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
    open_from_search(edit)?;
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_none())
        || SendMessageW(list, LB_GETCOUNT, 0, 0) != 4
        || GetFocus() != edit
        || !query(edit).is_empty()
    {
        return Err("Ctrl+K did not open the folder panel with only folder actions.".into());
    }
    SendMessageW(edit, WM_KEYDOWN, VK_ESCAPE as usize, 0);
    settle();
    if APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        app.context.is_some() || app.selected_identity() != folder_identity
    }) || query(edit) != "Documents"
        || GetFocus() != edit
        || IsWindowVisible(hwnd) == 0
    {
        return Err("Esc after Ctrl+K did not restore the original folder search.".into());
    }
    open_from_search(edit)?;
    SendMessageW(hwnd, WM_COMMAND, SETTINGS_BUTTON, 0); // Back
    settle();
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_some()) {
        return Err("Back did not leave the action panel.".into());
    }
    // No selection is a no-op, never an implicit activation.
    SendMessageW(list, LB_SETCURSEL, usize::MAX, 0);
    open_from_search(edit)?;
    if APP.with(|cell| cell.borrow().as_ref().unwrap().context.is_some())
        || query(edit) != "Documents"
        || SendMessageW(list, LB_GETCURSEL, 0, 0) != LB_ERR as isize
    {
        return Err("Ctrl+K with no selection changed the search.".into());
    }
    verify_unsupported_rows(hwnd, edit, list)?;
    verify_files_and_web(root, output, hwnd, edit, list)?;
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    Ok(())
}

fn same_app(a: &actions::Target, b: &actions::Target) -> bool {
    matches!((a, b), (actions::Target::App(a), actions::Target::App(b))
        if a.path == b.path && a.name == b.name)
}

/// Calculator copy and in-process Settings are deliberately safe if routing
/// regresses into activation. Never use URL, file, app or system-command rows.
unsafe fn verify_unsupported_rows(hwnd: HWND, edit: HWND, list: HWND) -> Result<(), String> {
    for (text, expected) in [
        ("= 6 * 7", Action::Copy("42".into())),
        ("> preferences", Action::Settings),
    ] {
        SetWindowTextW(edit, wide(text).as_ptr());
        settle();
        let (identity, settings_hwnd, supported_fixture) = APP.with(|cell| {
            let slot = cell.borrow();
            let app = slot.as_ref().unwrap();
            (
                app.selected_identity(),
                app.settings_hwnd,
                app.rows.first().map(|row| &row.action) == Some(&expected),
            )
        });
        if !supported_fixture
            || identity.is_none()
            || SendMessageW(list, LB_GETCOUNT, 0, 0) != 1
            || SendMessageW(list, LB_GETCURSEL, 0, 0) != 0
        {
            return Err(format!("Missing safe non-app/folder fixture for {text:?}."));
        }
        let top = SendMessageW(list, LB_GETTOPINDEX, 0, 0);
        open_from_search(edit)?;
        if query(edit) != text
            || APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.context.is_some()
                    || app.selected_identity() != identity
                    || app.settings_hwnd != settings_hwnd
            })
            || SendMessageW(list, LB_GETCURSEL, 0, 0) != 0
            || SendMessageW(list, LB_GETTOPINDEX, 0, 0) != top
            || GetFocus() != edit
            || IsWindowVisible(hwnd) == 0
        {
            return Err(format!(
                "Ctrl+K on {text:?} changed the query/selection or activated the row."
            ));
        }
    }
    Ok(())
}

unsafe fn verify_files_and_web(
    root: &Path,
    output: &Path,
    hwnd: HWND,
    edit: HWND,
    list: HWND,
) -> Result<(), String> {
    let file = root.join("context-panel-original.txt");
    let other = root.join("context-panel-replacement.txt");
    std::fs::write(&file, "Isolated context fixture").map_err(|e| e.to_string())?;
    std::fs::write(&other, "Never open this file").map_err(|e| e.to_string())?;
    let saved = APP.with(|cell| {
        let app = cell.borrow();
        let app = app.as_ref().unwrap();
        (
            app.settings.clone(),
            app.bookmarks.clone(),
            app.file_entries.clone(),
        )
    });
    let url = "https://example.com/panel/original?q=a%26b#fixture";
    let replacement = bookmarks::Bookmark {
        title: "Context bookmark replacement".into(),
        url: "https://example.com/panel/replacement".into(),
        source: "Isolated smoke fixture".into(),
    };
    let bookmark = bookmarks::Bookmark {
        title: "Context bookmark original".into(),
        url: url.into(),
        source: "Isolated smoke fixture".into(),
    };
    let result = (|| {
        for (case, query, expected, count, copy_id) in [
            (
                "file",
                "file: context-panel",
                file.to_string_lossy().into_owned(),
                3,
                actions::COPY_PATH,
            ),
            (
                "bookmark",
                "Context bookmark",
                url.into(),
                2,
                actions::COPY_URL,
            ),
            ("web", url, url.into(), 2, actions::COPY_URL),
        ] {
            // Seed only this smoke profile. No personal browser or disk scan.
            APP.with(|cell| {
                let mut slot = cell.borrow_mut();
                let app = slot.as_mut().unwrap();
                app.settings.bookmarks = true;
                app.settings.web_search = true;
                app.bookmarks = vec![bookmark.clone()];
                app.file_query = files::terms(query).unwrap_or("").into();
                app.file_pending = false;
                KillTimer(hwnd, FILE_TIMER);
                app.file_entries = if case == "file" {
                    vec![files::FileEntry { path: file.clone() }]
                } else {
                    vec![]
                };
            });
            SetWindowTextW(edit, wide(query).as_ptr());
            settle();
            let selected = APP
                .with(|cell| {
                    cell.borrow()
                        .as_ref()
                        .unwrap()
                        .rows
                        .iter()
                        .position(|r| match &r.action {
                            Action::File(path) => case == "file" && path == &file,
                            Action::Url(value) => case != "file" && value == url,
                            _ => false,
                        })
                })
                .ok_or_else(|| format!("Missing {case} context fixture."))?;
            SendMessageW(list, LB_SETCURSEL, selected, 0);
            let identity = APP.with(|cell| cell.borrow().as_ref().unwrap().selected_identity());
            actions::context_menu(list, -1);
            settle();
            let correct = APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.rows.len() == count
                    && app.rows.iter().all(|r| {
                        !matches!(
                            r.action,
                            Action::Context(
                                actions::ADMIN
                                    | actions::OTHER_USER
                                    | actions::TARGET
                                    | actions::HIDE
                            )
                        )
                    })
                    && match &app.context.as_ref().unwrap().target {
                        actions::Target::File(path) => case == "file" && path == &file,
                        actions::Target::Web(value) => case != "file" && value == url,
                        _ => false,
                    }
            });
            if !correct || GetFocus() != edit {
                return Err(format!("Wrong actions or captured target for {case}."));
            }
            // Deliver completed workers while the panel is open. The original
            // result now follows a different result, but the panel must not retarget.
            SendMessageW(list, LB_SETCURSEL, 1, 0);
            APP.with(|cell| {
                let mut slot = cell.borrow_mut();
                let app = slot.as_mut().unwrap();
                let (sender, receiver) = mpsc::sync_channel(1);
                sender
                    .send((
                        app.file_query.clone(),
                        Ok(vec![
                            files::FileEntry {
                                path: other.clone(),
                            },
                            files::FileEntry { path: file.clone() },
                        ]),
                    ))
                    .unwrap();
                app.file_worker = Some(receiver);
                app.files_ready();
                let (sender, receiver) = mpsc::channel();
                sender
                    .send(Ok(IndexResult {
                        apps: Scan {
                            entries: app.entries.clone(),
                            unreadable: 0,
                            truncated: false,
                        },
                        folders: folders::Scan {
                            entries: app.folders.clone(),
                            unreadable: 0,
                            limited: false,
                        },
                        bookmarks: bookmarks::Scan {
                            entries: vec![replacement.clone(), bookmark.clone()],
                            ..Default::default()
                        },
                    }))
                    .unwrap();
                app.worker = Some(receiver);
                app.reindex_again = false;
                app.index_ready();
                // App-only command IDs must be rejected, not just hidden.
                actions::perform(app, actions::ADMIN);
                actions::perform(app, actions::TARGET);
            });
            if SendMessageW(list, LB_GETCURSEL, 0, 0) != 1 {
                return Err(format!(
                    "Worker completion changed the {case} panel selection."
                ));
            }
            capture(
                hwnd,
                &output.join(format!("throw-inline-{case}-actions.bmp")),
            )?;
            SetWindowTextW(edit, wide("no matching action").as_ptr());
            settle();
            if SendMessageW(list, LB_GETCOUNT, 0, 0) != 0 {
                return Err(format!(
                    "{case} panel did not handle an empty filter result."
                ));
            }
            SetWindowTextW(edit, wide(" COPY ").as_ptr());
            settle();
            if !APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.rows.len() == 1 && app.rows[0].action == Action::Context(copy_id)
            }) {
                return Err(format!(
                    "{case} panel filtering exposed the wrong Copy action."
                ));
            }
            let settings = APP.with(|cell| cell.borrow().as_ref().unwrap().settings.clone());
            SendMessageW(edit, WM_KEYDOWN, VK_RETURN as usize, 0);
            if APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.context.is_none() || app.settings != settings
            }) || IsWindowVisible(hwnd) == 0
            {
                return Err(format!(
                    "{case} Copy closed the panel or changed app history."
                ));
            }
            SendMessageW(edit, EM_SETSEL, 0, -1);
            SendMessageW(edit, WM_PASTE, 0, 0);
            let mut pasted = [0u16; 4097];
            let length =
                GetWindowTextW(edit, pasted.as_mut_ptr(), pasted.len() as i32).max(0) as usize;
            if String::from_utf16_lossy(&pasted[..length]) != expected {
                return Err(format!("{case} Copy did not use the captured target."));
            }
            if case == "bookmark" {
                SendMessageW(hwnd, WM_COMMAND, SETTINGS_BUTTON, 0); // Back
            } else {
                SendMessageW(edit, WM_KEYDOWN, VK_ESCAPE as usize, 0);
            }
            settle();
            let mut text = [0u16; 513];
            let length = GetWindowTextW(edit, text.as_mut_ptr(), text.len() as i32).max(0) as usize;
            if String::from_utf16_lossy(&text[..length]) != query
                || APP.with(|cell| {
                    let slot = cell.borrow();
                    let app = slot.as_ref().unwrap();
                    app.context.is_some() || app.selected_identity() != identity
                })
            {
                return Err(format!(
                    "Back/Esc did not restore the {case} search and selection."
                ));
            }
        }
        Ok(())
    })();
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        if app.context.is_some() {
            app.close_context();
        }
        (app.settings, app.bookmarks, app.file_entries) = saved;
    });
    std::fs::remove_file(file).map_err(|e| e.to_string())?;
    std::fs::remove_file(other).map_err(|e| e.to_string())?;
    result
}
