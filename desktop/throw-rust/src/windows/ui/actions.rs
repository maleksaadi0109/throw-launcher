//! User-initiated app actions. Paths are data passed to Shell APIs, never commands.
use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};

const LAUNCH: usize = 1;
const ADMIN: usize = 2;
const OTHER_USER: usize = 3;
const CONTAINING: usize = 4;
const TARGET: usize = 5;
const COPY_PATH: usize = 6;
const HIDE: usize = 7;

pub(super) unsafe fn target_folder(path: &Path) -> Result<PathBuf, String> {
    if !path.is_absolute()
        || !path.is_file()
        || path.as_os_str().len() > 4096
        || path.to_string_lossy().contains('\0')
    {
        return Err("The app path is invalid or no longer exists.".into());
    }
    use ::windows::{
        core::{Interface, PCWSTR},
        Win32::{
            System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER, STGM_READ},
            UI::Shell::{IShellLinkW, ShellLink},
        },
    };
    let target = if path
        .extension()
        .is_some_and(|value| value.eq_ignore_ascii_case("lnk"))
    {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Cannot read shortcut: {e}"))?;
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        let normalized: PathBuf = path.components().collect();
        file.Load(PCWSTR(wide(normalized).as_ptr()), STGM_READ)
            .map_err(|e| format!("Cannot load shortcut: {e}"))?;
        let mut buffer = [0u16; 4096];
        // Get the saved filesystem target only. Do NOT resolve or search the disk,
        // invoke a link's arguments, or trigger Windows Installer repair.
        link.GetPath(&mut buffer, null_mut(), 0)
            .map_err(|e| format!("Cannot read shortcut target: {e}"))?;
        let length = buffer
            .iter()
            .position(|&c| c == 0)
            .ok_or("Target is too long.")?;
        if length == 0 {
            return Err("This shortcut has no filesystem target (for example a Store app).".into());
        }
        PathBuf::from(OsString::from_wide(&buffer[..length]))
    } else {
        path.to_path_buf()
    };
    if !target.is_absolute() || target.as_os_str().len() > 4096 {
        return Err("Shortcut target is not a valid absolute filesystem path.".into());
    }
    let folder = if target.is_dir() {
        target
    } else {
        target
            .parent()
            .ok_or("Target has no parent folder.")?
            .to_path_buf()
    };
    if !folder.is_dir() {
        return Err("The target folder no longer exists or is unavailable.".into());
    }
    Ok(folder)
}

unsafe fn launch(hwnd: HWND, entry: &Entry, verb: &'static str) -> Result<(), String> {
    if !entry.valid() || !entry.path.is_file() {
        return Err("This app shortcut no longer exists. Rebuild the app index.".into());
    }
    // Verbs are internal constants, not user input. No interpolated arguments.
    let normalized: PathBuf = entry.path.components().collect();
    let result = ShellExecuteW(
        hwnd,
        wide(verb).as_ptr(),
        wide(normalized).as_ptr(),
        null(),
        null(),
        SW_SHOWNORMAL,
    ) as isize;
    if result <= 32 {
        Err(format!("Windows could not perform this action (Shell error {result}). The app may not support this action, or the prompt was cancelled."))
    } else {
        Ok(())
    }
}

/// Release application state before entering the menu's nested message loop.
/// This lets indexing/icon completion and focus events continue normally.
pub(super) unsafe fn context_menu(hwnd: HWND, position: isize) {
    let snapshot = APP.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return None;
        };
        let app = slot.as_mut()?;
        if hwnd != app.list {
            return None;
        }
        let mut point = POINT {
            x: position as i16 as i32,
            y: (position >> 16) as i16 as i32,
        };
        if position != -1 {
            let mut local = point;
            ScreenToClient(hwnd, &mut local);
            let hit = SendMessageW(
                hwnd,
                LB_ITEMFROMPOINT,
                0,
                ((local.x as u16 as u32) | ((local.y as u16 as u32) << 16)) as isize,
            ) as u32;
            if hit >> 16 != 0 {
                return None;
            }
            let mut bounds: RECT = zeroed();
            if SendMessageW(
                hwnd,
                LB_GETITEMRECT,
                (hit & 0xffff) as usize,
                (&mut bounds as *mut RECT) as isize,
            ) == LB_ERR as isize
                || PtInRect(&bounds, local) == 0
            {
                return None;
            }
            SendMessageW(hwnd, LB_SETCURSEL, (hit & 0xffff) as usize, 0);
        } else {
            let selected = SendMessageW(hwnd, LB_GETCURSEL, 0, 0);
            if selected < 0 {
                return None;
            }
            let mut rect: RECT = zeroed();
            SendMessageW(
                hwnd,
                LB_GETITEMRECT,
                selected as usize,
                (&mut rect as *mut RECT) as isize,
            );
            point = POINT {
                x: rect.left + app.px(64),
                y: rect.bottom,
            };
            ClientToScreen(hwnd, &mut point);
        }
        let selected = SendMessageW(hwnd, LB_GETCURSEL, 0, 0);
        let row = app.rows.get(selected as usize)?;
        let (entry, folder) = match &row.action {
            Action::App(index) => (app.entries.get(*index)?.clone(), false),
            Action::Folder(path) => (
                Entry {
                    name: row.title.clone(),
                    path: path.clone(),
                },
                true,
            ),
            _ => return None,
        };
        InvalidateRect(app.list, null(), 0);
        app.popup = true;
        Some((app.hwnd, entry, folder, point))
    });
    let Some((owner, entry, folder, point)) = snapshot else {
        return;
    };
    let menu = build_menu_for(folder);
    let failed = menu.is_null();
    let command = if failed {
        0
    } else {
        SetForegroundWindow(owner);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            point.x,
            point.y,
            0,
            owner,
            null(),
        );
        DestroyMenu(menu);
        command as usize
    };
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.popup = false;
        }
    });
    if failed {
        message(owner, "Windows could not create the app action menu.", true);
    }
    if command == 0 {
        return;
    }
    let result = match command {
        LAUNCH if folder => {
            if Folder::new(entry.path.clone()).is_some() && entry.path.is_dir() {
                open(owner, entry.path.as_os_str())
            } else {
                Err("This folder is no longer available.".into())
            }
        }
        LAUNCH => launch(owner, &entry, "open"),
        ADMIN => launch(owner, &entry, "runas"),
        OTHER_USER => launch(owner, &entry, "runasuser"),
        CONTAINING | TARGET => {
            if !entry.valid() && !(folder && Folder::new(entry.path.clone()).is_some()) {
                Err("Invalid app path. Rebuild the index.".into())
            } else {
                let folder = if command == TARGET {
                    target_folder(&entry.path)
                } else {
                    entry
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or("No containing folder.".into())
                };
                folder.and_then(|folder| {
                    if !folder.is_dir() {
                        return Err("The folder is unavailable.".into());
                    }
                    open(owner, folder.as_os_str())
                })
            }
        }
        COPY_PATH => copy_text(owner, &entry.path.to_string_lossy()),
        HIDE => Ok(()),
        _ => return,
    };
    match result {
        Ok(()) => {
            if !folder && [LAUNCH, ADMIN, OTHER_USER].contains(&command) {
                APP.with(|cell| {
                    if let Some(app) = cell.borrow_mut().as_mut() {
                        app.settings
                            .record_launch(entry.path.to_string_lossy().into_owned());
                        if let Err(error) = app.settings.save(&app.root.join("settings.json")) {
                            app.set_status(&format!(
                                "Opened app, but history was not saved: {error}"
                            ));
                        }
                    }
                });
            }
            if command != COPY_PATH {
                ShowWindow(owner, SW_HIDE);
            }
        }
        Err(error) => message(owner, &error, true),
    }
}

pub(super) unsafe fn build_menu() -> HMENU {
    build_menu_for(false)
}

pub(super) unsafe fn build_menu_for(folder: bool) -> HMENU {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return menu;
    }
    for (id, title) in [
        (LAUNCH, "Open"),
        (ADMIN, "Run as administrator"),
        (OTHER_USER, "Run as different user"),
        (CONTAINING, "Open containing folder"),
        (TARGET, "Open target folder"),
        (COPY_PATH, "Copy path"),
        (HIDE, "Hide launcher"),
    ] {
        if folder && [ADMIN, OTHER_USER, TARGET].contains(&id) {
            continue;
        }
        if AppendMenuW(menu, MF_STRING, id, wide(title).as_ptr()) == 0 {
            DestroyMenu(menu);
            return null_mut();
        }
    }
    menu
}

/// Native edit commands work even when keyboard focus is on the result list.
pub(super) unsafe fn text_shortcut(edit: HWND, key: usize) -> bool {
    let command = match key as u8 {
        b'A' => {
            SetFocus(edit);
            SendMessageW(edit, EM_SETSEL, 0, -1);
            return true;
        }
        b'C' => WM_COPY,
        b'X' => WM_CUT,
        b'V' => WM_PASTE,
        b'Z' => WM_UNDO,
        _ => return false,
    };
    SetFocus(edit);
    SendMessageW(edit, command, 0, 0);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_target_is_read_without_launching_it() {
        use ::windows::{
            core::{Interface, PCWSTR},
            Win32::{
                System::Com::{CoCreateInstance, IPersistFile, CLSCTX_INPROC_SERVER},
                UI::Shell::{IShellLinkW, ShellLink},
            },
        };
        unsafe {
            let _com = ShellApartment::initialize().unwrap();
            let root = std::env::temp_dir().join(format!("ThrowLinkTest-{}", std::process::id()));
            std::fs::create_dir_all(&root).unwrap();
            let executable = std::env::current_exe().unwrap();
            let link: IShellLinkW =
                CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            link.SetPath(PCWSTR(wide(&executable).as_ptr())).unwrap();
            let file: IPersistFile = link.cast().unwrap();
            let shortcut = root.join("App.lnk");
            file.Save(PCWSTR(wide(&shortcut).as_ptr()), true).unwrap();
            assert_eq!(
                target_folder(&shortcut).unwrap(),
                executable.parent().unwrap()
            );
            assert_eq!(
                target_folder(&executable).unwrap(),
                executable.parent().unwrap()
            );
            assert!(target_folder(Path::new("relative.exe")).is_err());
            assert!(target_folder(&root.join("missing.lnk")).is_err());
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
