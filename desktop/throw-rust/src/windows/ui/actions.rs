//! Inline actions. A captured filesystem target cannot change as indexes update.
use super::*;
use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::Path};

pub(super) const LAUNCH: usize = 1;
pub(super) const ADMIN: usize = 2;
pub(super) const OTHER_USER: usize = 3;
pub(super) const CONTAINING: usize = 4;
pub(super) const TARGET: usize = 5;
pub(super) const COPY_PATH: usize = 6;
pub(super) const HIDE: usize = 7;

pub(super) struct Panel {
    pub entry: Entry,
    folder: bool,
    query: String,
    identity: Option<String>,
    top: usize,
    filter: String,
}

pub(super) fn rows(folder: bool, filter: &str) -> Vec<ResultRow> {
    let filter = filter.trim().to_lowercase();
    [
        (OTHER_USER, "Run as different user"),
        (ADMIN, "Run as administrator"),
        (CONTAINING, "Open containing folder"),
        (TARGET, "Open target folder"),
        (HIDE, "Hide launcher"),
        (COPY_PATH, "Copy path"),
        (LAUNCH, "Open"),
    ]
    .into_iter()
    .filter(|(id, title)| {
        !(folder && [ADMIN, OTHER_USER, TARGET].contains(id))
            && title.to_lowercase().contains(&filter)
    })
    .map(|(id, title)| ResultRow {
        title: title.into(),
        detail: String::new(),
        action: Action::Context(id),
    })
    .collect()
}

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
        .is_some_and(|v| v.eq_ignore_ascii_case("lnk"))
    {
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("Cannot read shortcut: {e}"))?;
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        let normalized: PathBuf = path.components().collect();
        file.Load(PCWSTR(wide(normalized).as_ptr()), STGM_READ)
            .map_err(|e| e.to_string())?;
        let mut buffer = [0u16; 4096];
        // Read the saved target. Never resolve links, launch arguments or repair MSI.
        link.GetPath(&mut buffer, null_mut(), 0)
            .map_err(|e| e.to_string())?;
        let length = buffer
            .iter()
            .position(|&c| c == 0)
            .ok_or("Target is too long.")?;
        if length == 0 {
            return Err("This shortcut has no filesystem target.".into());
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
        return Err("The target folder is unavailable.".into());
    }
    Ok(folder)
}

unsafe fn launch(hwnd: HWND, entry: &Entry, verb: &'static str) -> Result<(), String> {
    if !entry.valid() || !entry.path.is_file() {
        return Err("This app shortcut no longer exists. Rebuild the app index.".into());
    }
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
        Err(format!("Windows could not perform this action (Shell error {result}). The app may not support it, or the permission prompt was cancelled."))
    } else {
        Ok(())
    }
}

/// Right click, Shift+F10 and the application/menu key all use the same panel.
pub(super) unsafe fn context_menu(hwnd: HWND, position: isize) {
    APP.with(|cell| {
        let Ok(mut slot) = cell.try_borrow_mut() else {
            return;
        };
        let Some(app) = slot.as_mut() else {
            return;
        };
        if hwnd != app.list || app.context.is_some() {
            return;
        }
        if position != -1 {
            let mut point = POINT {
                x: position as i16 as i32,
                y: (position >> 16) as i16 as i32,
            };
            ScreenToClient(hwnd, &mut point);
            let hit = SendMessageW(
                hwnd,
                LB_ITEMFROMPOINT,
                0,
                ((point.x as u16 as u32) | ((point.y as u16 as u32) << 16)) as isize,
            ) as u32;
            let mut bounds: RECT = zeroed();
            if hit >> 16 != 0
                || SendMessageW(
                    hwnd,
                    LB_GETITEMRECT,
                    (hit & 0xffff) as usize,
                    (&mut bounds as *mut RECT) as isize,
                ) == LB_ERR as isize
                || PtInRect(&bounds, point) == 0
            {
                return;
            }
            SendMessageW(hwnd, LB_SETCURSEL, (hit & 0xffff) as usize, 0);
        }
        let selected = SendMessageW(hwnd, LB_GETCURSEL, 0, 0);
        let Some(row) = app.rows.get(selected as usize) else {
            return;
        };
        let (entry, folder) = match &row.action {
            Action::App(index) => {
                let Some(entry) = app.entries.get(*index) else {
                    return;
                };
                (entry.clone(), false)
            }
            Action::Folder(path) => (
                Entry {
                    name: row.title.clone(),
                    path: path.clone(),
                },
                true,
            ),
            _ => return,
        };
        let mut query = [0u16; 513];
        let length =
            GetWindowTextW(app.edit, query.as_mut_ptr(), query.len() as i32).max(0) as usize;
        app.context = Some(Panel {
            entry,
            folder,
            query: String::from_utf16_lossy(&query[..length]),
            identity: app.selected_identity(),
            top: SendMessageW(app.list, LB_GETTOPINDEX, 0, 0).max(0) as usize,
            filter: String::new(),
        });
        SetWindowTextW(app.edit, wide("").as_ptr());
        SetWindowTextW(
            GetDlgItem(app.hwnd, SETTINGS_BUTTON as i32),
            wide("Back").as_ptr(),
        );
        app.refresh();
        SetFocus(app.edit);
    });
}

impl App {
    pub(super) unsafe fn refresh_context(&mut self) {
        let selected = SendMessageW(self.list, LB_GETCURSEL, 0, 0);
        let selected_action = self.rows.get(selected as usize).map(|r| r.action.clone());
        let old_top = SendMessageW(self.list, LB_GETTOPINDEX, 0, 0).max(0) as usize;
        let Some(panel) = self.context.as_mut() else {
            return;
        };
        let mut query = [0u16; 513];
        let length =
            GetWindowTextW(self.edit, query.as_mut_ptr(), query.len() as i32).max(0) as usize;
        let filter = String::from_utf16_lossy(&query[..length]);
        let same_filter = filter == panel.filter;
        panel.filter = filter.clone();
        self.rows = rows(panel.folder, &filter);
        let name = panel.entry.name.clone();
        SendMessageW(self.list, WM_SETREDRAW, 0, 0);
        SendMessageW(self.list, LB_RESETCONTENT, 0, 0);
        for row in &self.rows {
            SendMessageW(
                self.list,
                LB_ADDSTRING,
                0,
                wide(&row.title).as_ptr() as isize,
            );
        }
        let selection = if same_filter {
            self.rows
                .iter()
                .position(|r| Some(&r.action) == selected_action.as_ref())
                .unwrap_or(0)
        } else {
            0
        };
        SendMessageW(self.list, LB_SETCURSEL, selection, 0);
        if same_filter {
            SendMessageW(self.list, LB_SETTOPINDEX, old_top, 0);
        }
        ShowWindow(
            self.list,
            if self.rows.is_empty() {
                SW_HIDE
            } else {
                SW_SHOW
            },
        );
        SendMessageW(self.list, WM_SETREDRAW, 1, 0);
        self.set_status(&format!("Actions for {name}"));
        PostMessageW(self.hwnd, FIT_RESULTS, 0, 0);
        RedrawWindow(
            self.hwnd,
            null(),
            null_mut(),
            RDW_INVALIDATE | RDW_ALLCHILDREN,
        );
    }

    pub(super) unsafe fn close_context(&mut self) {
        let Some(panel) = self.context.take() else {
            return;
        };
        SetWindowTextW(
            GetDlgItem(self.hwnd, SETTINGS_BUTTON as i32),
            wide("Settings").as_ptr(),
        );
        SetWindowTextW(self.edit, wide(panel.query).as_ptr());
        self.refresh();
        self.restore_selection(panel.identity);
        SendMessageW(self.list, LB_SETTOPINDEX, panel.top, 0);
        SetFocus(self.edit);
    }
}

pub(super) unsafe fn perform(app: &mut App, command: usize) {
    let Some(panel) = app.context.as_ref() else {
        return;
    };
    if !rows(panel.folder, "")
        .iter()
        .any(|r| r.action == Action::Context(command))
    {
        return;
    }
    let entry = panel.entry.clone();
    let folder = panel.folder;
    let owner = app.hwnd;
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
                let target = if command == TARGET {
                    target_folder(&entry.path)
                } else {
                    entry
                        .path
                        .parent()
                        .map(Path::to_path_buf)
                        .ok_or("No containing folder.".into())
                };
                target.and_then(|path| {
                    if path.is_dir() {
                        open(owner, path.as_os_str())
                    } else {
                        Err("The folder is unavailable.".into())
                    }
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
                app.settings
                    .record_launch(entry.path.to_string_lossy().into_owned());
                if let Err(error) = app.settings.save(&app.root.join("settings.json")) {
                    app.set_status(&format!("Opened app, but history was not saved: {error}"));
                }
            }
            if command == COPY_PATH {
                app.set_status("Path copied. Esc to return to search.");
            } else {
                app.close_context();
                ShowWindow(owner, SW_HIDE);
            }
        }
        Err(error) => {
            app.popup = true;
            message(owner, &error, true);
            app.popup = false;
        }
    }
}

/// Keep native editing shortcuts when focus is on the result list.
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
    fn panel_actions_are_filtered_without_changing_ids() {
        assert_eq!(rows(false, "").len(), 7);
        assert_eq!(rows(true, "").len(), 4);
        assert_eq!(rows(false, "copy")[0].action, Action::Context(COPY_PATH));
        assert!(rows(true, "")
            .iter()
            .all(|r| !matches!(r.action, Action::Context(ADMIN | OTHER_USER | TARGET))));
    }
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
