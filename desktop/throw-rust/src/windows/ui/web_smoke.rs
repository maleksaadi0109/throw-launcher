//! Native control smoke coverage, using only the smoke profile and browser fixtures.
use super::web_settings::*;
use super::*;
use std::{
    path::Path,
    time::{Duration, Instant},
};

unsafe fn settle() {
    let mut msg: MSG = zeroed();
    while PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) != 0 {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

unsafe fn set(parent: HWND, id: usize, text: &str) {
    SetWindowTextW(GetDlgItem(parent, id as i32), wide(text).as_ptr());
}

unsafe fn check(parent: HWND, id: usize, value: bool) {
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

fn require(value: bool, error: &str) -> Result<(), String> {
    if value {
        Ok(())
    } else {
        Err(error.into())
    }
}

pub(super) unsafe fn verify(root: &Path) -> Result<(), String> {
    let original = APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.show_settings();
        app.select_settings_section(2);
        app.settings.clone()
    });
    let settings_hwnd = APP.with(|cell| cell.borrow().as_ref().unwrap().settings_hwnd);
    SendMessageW(GetDlgItem(settings_hwnd, LIMIT as i32), CB_SETCURSEL, 2, 0);
    SendMessageW(settings_hwnd, WM_COMMAND, WEB_MANAGER, 0);
    let (web, edit, list, browser_root) = APP.with(|cell| {
        let slot = cell.borrow();
        let app = slot.as_ref().unwrap();
        (
            app.web_hwnd,
            app.edit,
            app.list,
            app.browser_root.clone().unwrap(),
        )
    });
    require(
        !web.is_null() && !GetDlgItem(web, STORE as i32).is_null(),
        "Native web settings controls are missing.",
    )?;
    require(
        SendMessageW(GetDlgItem(web, BOOKMARKS as i32), BM_GETCHECK, 0, 0)
            == BST_UNCHECKED as isize,
        "Bookmarks are not opt-in.",
    )?;
    SendMessageW(web, WM_COMMAND, NEW, 0);
    set(web, NAME, "Smoke Search");
    set(web, KEYWORD, "smoke");
    set(web, TEMPLATE, "https://example.com/search?q={query}");
    SendMessageW(web, WM_COMMAND, STORE, 0);
    let path = root.join("settings.json");
    let saved = Settings::load(&path)?;
    require(
        APP.with(|cell| {
            cell.borrow()
                .as_ref()
                .unwrap()
                .settings_draft()
                .map(|s| s.max_results == 100)
                .unwrap_or(false)
        }),
        "Saving a provider discarded an unrelated Settings draft.",
    )?;
    require(
        saved.web_providers.len() == original.web_providers.len() + 1,
        "Native provider create was not persisted.",
    )?;
    set(web, KEYWORD, "docs");
    SendMessageW(web, WM_COMMAND, STORE, 0);
    require(
        Settings::load(&path)?.web_providers.last().unwrap().keyword == "docs",
        "Native provider edit was not persisted.",
    )?;
    SetWindowTextW(edit, wide("DOCS a&b é").as_ptr());
    settle();
    require(
        SendMessageW(list, LB_GETCOUNT, 0, 0) == 1
            && APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                !app.file_pending
                    && app.file_query.is_empty()
                    && matches!(app.rows.first().map(|r| &r.action),
            Some(Action::Url(url)) if url == "https://example.com/search?q=a%26b%20%C3%A9")
            }),
        "Native keyword search did not bind an encoded HTTP(S) action.",
    )?;
    let before = std::fs::read(&path).map_err(|e| e.to_string())?;
    set(web, TEMPLATE, "file:///{query}");
    let invalid = APP.with(|cell| cell.borrow_mut().as_mut().unwrap().save_provider());
    require(
        invalid.is_err() && std::fs::read(&path).map_err(|e| e.to_string())? == before,
        "Invalid provider changed saved settings.",
    )?;
    set(web, TEMPLATE, "https://example.com/search?q={query}");
    // Simulate a failed replacement: both the live candidate and old file must survive.
    let backup = root.join("settings-smoke-backup.json");
    std::fs::rename(&path, &backup).map_err(|e| e.to_string())?;
    std::fs::create_dir(&path).map_err(|e| e.to_string())?;
    set(web, KEYWORD, "unsaved");
    let failed = APP.with(|cell| cell.borrow_mut().as_mut().unwrap().save_provider());
    std::fs::remove_dir(&path).map_err(|e| e.to_string())?;
    std::fs::rename(&backup, &path).map_err(|e| e.to_string())?;
    require(
        failed.is_err()
            && APP.with(|cell| {
                cell.borrow().as_ref().unwrap().settings == Settings::load(&path).unwrap()
            }),
        "Failed save changed live provider settings.",
    )?;
    set(web, KEYWORD, "docs");
    check(web, ENABLED, false);
    SendMessageW(web, WM_COMMAND, STORE, 0);
    SetWindowTextW(edit, wide("docs uniquely-missing").as_ptr());
    settle();
    require(
        SendMessageW(list, LB_GETCOUNT, 0, 0) == 0
            && !Settings::load(&path)?.web_providers.last().unwrap().enabled,
        "Provider disable did not take effect.",
    )?;
    check(web, WEB_ENABLED, false);
    SendMessageW(web, WM_COMMAND, APPLY, 0);
    SetWindowTextW(edit, wide("? uniquely-missing").as_ptr());
    settle();
    require(
        SendMessageW(list, LB_GETCOUNT, 0, 0) == 0 && !Settings::load(&path)?.web_search,
        "Global web switch did not persist.",
    )?;
    check(web, WEB_ENABLED, true);
    SendMessageW(web, WM_COMMAND, APPLY, 0);

    // Confirm the real Delete button's prompt on a separate native thread.
    let dismiss = std::thread::spawn(|| {
        let deadline = Instant::now() + Duration::from_secs(3);
        while Instant::now() < deadline {
            let dialog = FindWindowW(
                wide("#32770").as_ptr(),
                wide("Throw · Delete provider").as_ptr(),
            );
            if !dialog.is_null() {
                SendMessageW(dialog, WM_COMMAND, IDYES as usize, 0);
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    });
    SendMessageW(web, WM_COMMAND, DELETE, 0);
    require(
        dismiss.join().unwrap_or(false)
            && Settings::load(&path)?.web_providers == original.web_providers,
        "Native provider delete did not persist.",
    )?;

    let candidates = bookmarks::candidates(&browser_root);
    std::fs::create_dir_all(candidates[0].0.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&candidates[0].0, br#"{"roots":{"bookmark_bar":{"type":"folder","children":[{"type":"url","name":"Smoke bookmark fixture","url":"https://example.com/bookmark"},{"type":"url","name":"Unsafe bookmark fixture","url":"javascript:alert(1)"}]}}}"#).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(candidates[1].0.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(&candidates[1].0, b"invalid").map_err(|e| e.to_string())?;
    let scan = bookmarks::discover(&browser_root, true);
    require(
        scan.entries.len() == 1 && scan.errors.len() == 1 && scan.skipped == 1,
        "Windows bookmark fixture discovery/validation failed.",
    )?;
    // As in the folder arrival smoke test, deliver a deterministic completed
    // worker result. Do not scan personal drives or browsers during this test.
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
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
                bookmarks: scan,
            }))
            .unwrap();
        app.worker = Some(receiver);
    });
    check(web, BOOKMARKS, true);
    SendMessageW(web, WM_COMMAND, APPLY, 0);
    require(
        Settings::load(&path)?.bookmarks,
        "Bookmark opt-in was not persisted.",
    )?;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.reindex_again = false;
        app.index_ready();
    });
    SetWindowTextW(edit, wide("Smoke bookmark fixture").as_ptr());
    settle();
    require(
        SendMessageW(list, LB_GETCOUNT, 0, 0) == 1
            && APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.bookmark_status.contains("error")
                    && matches!(app.rows.first().map(|r| &r.action),
            Some(Action::Url(url)) if url == "https://example.com/bookmark")
            }),
        "Native bookmark action or discovery errors are missing.",
    )?;
    check(web, BOOKMARKS, false);
    SendMessageW(web, WM_COMMAND, APPLY, 0);
    require(
        !Settings::load(&path)?.bookmarks
            && APP.with(|cell| {
                let slot = cell.borrow();
                let app = slot.as_ref().unwrap();
                app.bookmarks.is_empty() && app.rows.is_empty()
            }),
        "Disabling bookmarks left searchable results.",
    )?;
    RedrawWindow(
        web,
        null(),
        null_mut(),
        RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW | RDW_ALLCHILDREN,
    );
    let output = std::env::current_exe().map_err(|e| e.to_string())?;
    super::snapshot::capture(
        web,
        &output
            .parent()
            .ok_or("Missing executable directory")?
            .join("throw-web-settings.bmp"),
    )?;
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.as_mut().unwrap();
        app.settings = original;
        app.settings.save(&path)?;
        DestroyWindow(web);
        app.web_hwnd = null_mut();
        app.close_control_center();
        Ok::<_, String>(())
    })?;
    SetWindowTextW(edit, wide("").as_ptr());
    settle();
    Ok(())
}
