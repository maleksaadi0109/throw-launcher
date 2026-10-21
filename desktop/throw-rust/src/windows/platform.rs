use std::{
    ffi::OsStr,
    mem::{size_of, zeroed},
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr::{null, null_mut},
};
use throw_launcher::{
    config::{atomic_write, read_bounded, Browser},
    index::{Entry, STANDARD_INDEX_LIMIT},
};
use windows_sys::Win32::{
    Foundation::*,
    System::{Com::*, DataExchange::*, Memory::*, ProcessStatus::*, Registry::*, Threading::*},
    UI::{Controls::Dialogs::*, Shell::*, WindowsAndMessaging::*},
};

pub fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
    value.as_ref().encode_wide().chain(Some(0)).collect()
}

pub fn profile(portable: bool) -> Result<PathBuf, String> {
    let root = if portable {
        std::env::current_exe()
            .map_err(|e| e.to_string())?
            .parent()
            .ok_or("Executable has no parent")?
            .join("ThrowRustData")
    } else {
        PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is missing.")?)
            .join("ThrowLauncherRust")
    };
    std::fs::create_dir_all(&root).map_err(|e| format!("Cannot create Rust profile: {e}"))?;
    Ok(root)
}

pub struct ShellApartment;
impl ShellApartment {
    pub unsafe fn initialize() -> Result<Self, String> {
        let status = CoInitializeEx(
            null(),
            (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
        );
        if status < 0 {
            Err(format!("Cannot initialize Windows Shell ({status:#x})."))
        } else {
            Ok(Self)
        }
    }
}
impl Drop for ShellApartment {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}

pub fn app_roots() -> Result<Vec<PathBuf>, String> {
    use std::os::windows::ffi::OsStringExt;
    let mut roots = Vec::new();
    for (id, name) in [
        (FOLDERID_Programs, "user Start menu"),
        (FOLDERID_CommonPrograms, "shared Start menu"),
        (FOLDERID_Desktop, "Desktop"),
        (FOLDERID_PublicDesktop, "public Desktop"),
    ] {
        unsafe {
            let mut pointer = null_mut();
            let status =
                SHGetKnownFolderPath(&id, KF_FLAG_DONT_VERIFY as u32, null_mut(), &mut pointer);
            if status < 0 || pointer.is_null() {
                return Err(format!("Cannot locate {name} ({status:#x})."));
            }
            let mut length = 0;
            while length < 32768 && *pointer.add(length) != 0 {
                length += 1;
            }
            let path = if length < 32768 {
                Some(PathBuf::from(std::ffi::OsString::from_wide(
                    std::slice::from_raw_parts(pointer, length),
                )))
            } else {
                None
            };
            CoTaskMemFree(pointer.cast());
            roots.push(path.ok_or("Known folder path exceeds Windows limits.")?);
        }
    }
    Ok(roots)
}

pub fn load_index(path: &Path, limit: usize) -> Result<Vec<Entry>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = read_bounded(path, 4 * 1024 * 1024).map_err(|e| e.to_string())?;
    let mut entries: Vec<Entry> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if entries.len() > STANDARD_INDEX_LIMIT || entries.iter().any(|e| !e.valid()) {
        return Err("Cached app index has invalid paths or too many entries; rebuild it.".into());
    }
    entries.truncate(limit);
    Ok(entries)
}

pub fn folder_roots() -> Result<Vec<PathBuf>, String> {
    use std::os::windows::ffi::OsStringExt;
    let mut roots = Vec::new();
    unsafe {
        for id in [
            FOLDERID_Profile,
            FOLDERID_Desktop,
            FOLDERID_Documents,
            FOLDERID_Downloads,
            FOLDERID_Pictures,
            FOLDERID_Music,
            FOLDERID_Videos,
        ] {
            let mut pointer = null_mut();
            if SHGetKnownFolderPath(&id, KF_FLAG_DONT_VERIFY as u32, null_mut(), &mut pointer) >= 0
                && !pointer.is_null()
            {
                let mut length = 0;
                while length < 32768 && *pointer.add(length) != 0 {
                    length += 1;
                }
                if length < 32768 {
                    let path = PathBuf::from(std::ffi::OsString::from_wide(
                        std::slice::from_raw_parts(pointer, length),
                    ));
                    if throw_launcher::folders::Folder::new(path.clone()).is_some()
                        && !roots.contains(&path)
                    {
                        roots.push(path);
                    }
                }
                CoTaskMemFree(pointer.cast());
            }
        }
        use windows_sys::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
        let drives = GetLogicalDrives();
        for letter in b'A'..=b'Z' {
            if drives & (1 << (letter - b'A')) != 0 {
                let path = PathBuf::from(format!("{}:\\", letter as char));
                if GetDriveTypeW(wide(&path).as_ptr()) == 3 {
                    roots.push(path);
                }
            }
        }
    }
    if roots.is_empty() {
        Err("Windows did not provide any folder search locations.".into())
    } else {
        Ok(roots)
    }
}

pub fn save_index(path: &Path, entries: &[Entry]) -> Result<(), String> {
    let bytes = serde_json::to_vec(entries).map_err(|e| e.to_string())?;
    // Use the same bound for reads and writes, including unusually long paths.
    if bytes.len() > 4 * 1024 * 1024 {
        return Err("Index exceeds the 4 MB disk limit.".into());
    }
    atomic_write(path, &bytes).map_err(|e| e.to_string())
}

pub unsafe fn open(hwnd: HWND, target: &OsStr) -> Result<(), String> {
    // A filename or an allowlisted HTTP(S) URL, NOT a command line. No cmd.exe.
    let result = ShellExecuteW(
        hwnd,
        wide("open").as_ptr(),
        wide(target).as_ptr(),
        null(),
        null(),
        SW_SHOWNORMAL,
    );
    if result as isize <= 32 {
        Err(format!(
            "Windows could not open this item (ShellExecute error {}).",
            result as isize
        ))
    } else {
        Ok(())
    }
}

/// Only direct HTTP(S) input uses this preference. Never pass a command line
/// through ShellExecute or resolve Chrome through the current directory/PATH.
pub unsafe fn open_direct_url(hwnd: HWND, url: &str, browser: Browser) -> Result<(), String> {
    let url = throw_launcher::web::safe_url(url).ok_or("URL is not allowed.")?;
    match browser {
        Browser::WindowsDefault => open(hwnd, OsStr::new(&url)),
        Browser::Chrome => {
            let executable = chrome_path().ok_or(
                "Google Chrome was not found. Install Chrome or choose Windows default in Settings > General.",
            )?;
            std::process::Command::new(executable)
                .arg(&url)
                .spawn()
                .map_err(|e| format!("Could not open Google Chrome: {e}. Choose Windows default in Settings > General to use another browser."))?;
            Ok(())
        }
    }
}

fn valid_chrome_path(path: &Path) -> bool {
    path.is_absolute()
        && path
            .file_name()
            .and_then(OsStr::to_str)
            .is_some_and(|name| name.eq_ignore_ascii_case("chrome.exe"))
        && path.is_file()
}

unsafe fn chrome_path() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    // App Paths supports registered custom installs as well as per-user installs.
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [RRF_SUBKEY_WOW6464KEY, RRF_SUBKEY_WOW6432KEY] {
            let mut buffer = vec![0u16; 32768];
            let mut bytes = (buffer.len() * 2) as u32;
            let status = RegGetValueW(
                root,
                wide(r"Software\Microsoft\Windows\CurrentVersion\App Paths\chrome.exe").as_ptr(),
                null(),
                RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ | view,
                null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            );
            if status == ERROR_SUCCESS {
                let Some(length) = buffer.iter().position(|&c| c == 0) else {
                    continue;
                };
                let mut value = &buffer[..length];
                // Some registrations surround the executable path with quotes.
                if value.len() >= 2
                    && value[0] == b'"' as u16
                    && value[value.len() - 1] == b'"' as u16
                {
                    value = &value[1..value.len() - 1];
                }
                let path = PathBuf::from(std::ffi::OsString::from_wide(value));
                if valid_chrome_path(&path) {
                    return Some(path);
                }
            }
        }
    }
    for variable in [
        "LOCALAPPDATA",
        "ProgramW6432",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
    ] {
        if let Some(root) = std::env::var_os(variable) {
            let path = PathBuf::from(root).join(r"Google\Chrome\Application\chrome.exe");
            if valid_chrome_path(&path) {
                return Some(path);
            }
        }
    }
    None
}

pub fn validate_file(path: &Path) -> Result<(), String> {
    let entry = throw_launcher::files::FileEntry { path: path.into() };
    if !entry.valid() || !path.is_file() {
        Err("This file was moved, deleted, or cannot be accessed. Search again or check its folder permissions.".into())
    } else {
        Ok(())
    }
}
pub unsafe fn copy_text(hwnd: HWND, value: &str) -> Result<(), String> {
    let text = wide(value);
    let memory = GlobalAlloc(GMEM_MOVEABLE, text.len() * 2);
    if memory.is_null() {
        return Err("Cannot allocate clipboard memory.".into());
    }
    let pointer = GlobalLock(memory);
    if pointer.is_null() {
        GlobalFree(memory);
        return Err("Cannot lock clipboard memory.".into());
    }
    std::ptr::copy_nonoverlapping(text.as_ptr(), pointer.cast::<u16>(), text.len());
    GlobalUnlock(memory);
    if OpenClipboard(hwnd) == 0 {
        GlobalFree(memory);
        return Err("Clipboard is busy. Try again.".into());
    }
    let result = if EmptyClipboard() != 0 && !SetClipboardData(13, memory).is_null() {
        Ok(()) // Windows now owns the allocation.
    } else {
        GlobalFree(memory);
        Err("Cannot copy to clipboard.".into())
    };
    CloseClipboard();
    result
}

pub unsafe fn memory_report() -> Result<String, String> {
    let mut counters: PROCESS_MEMORY_COUNTERS_EX = zeroed();
    counters.cb = size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
    if GetProcessMemoryInfo(
        GetCurrentProcess(),
        (&mut counters as *mut PROCESS_MEMORY_COUNTERS_EX).cast(),
        counters.cb,
    ) == 0
    {
        return Err("Windows could not read process memory.".into());
    }
    Ok(format!(
        "Working set: {:.1} MB   •   Private committed: {:.1} MB",
        counters.WorkingSetSize as f64 / 1_048_576.0,
        counters.PrivateUsage as f64 / 1_048_576.0
    ))
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_NAME: &str = "ThrowLauncherRust";

pub unsafe fn startup_enabled() -> Result<bool, String> {
    let mut key = null_mut();
    let result = RegOpenKeyExW(
        HKEY_CURRENT_USER,
        wide(RUN_KEY).as_ptr(),
        0,
        KEY_QUERY_VALUE,
        &mut key,
    );
    if result == ERROR_FILE_NOT_FOUND {
        return Ok(false);
    }
    if result != ERROR_SUCCESS {
        return Err(format!("Cannot inspect startup registration ({result})."));
    }
    let status = RegQueryValueExW(
        key,
        wide(STARTUP_NAME).as_ptr(),
        null(),
        null_mut(),
        null_mut(),
        null_mut(),
    );
    RegCloseKey(key);
    match status {
        ERROR_SUCCESS => Ok(true),
        ERROR_FILE_NOT_FOUND => Ok(false),
        _ => Err(format!("Cannot inspect startup entry ({status}).")),
    }
}

pub unsafe fn set_startup(enabled: bool, portable: bool) -> Result<(), String> {
    let mut key = null_mut();
    let result = RegCreateKeyExW(
        HKEY_CURRENT_USER,
        wide(RUN_KEY).as_ptr(),
        0,
        null(),
        0,
        KEY_SET_VALUE,
        null(),
        &mut key,
        null_mut(),
    );
    if result != ERROR_SUCCESS {
        return Err(format!("Cannot open startup registration ({result})."));
    }
    let status = if enabled {
        let path = std::env::current_exe().map_err(|e| e.to_string())?;
        let command = format!(
            "\"{}\" --background{}",
            path.display(),
            if portable { " --portable" } else { "" }
        );
        let value = wide(command);
        RegSetValueExW(
            key,
            wide(STARTUP_NAME).as_ptr(),
            0,
            REG_SZ,
            value.as_ptr().cast(),
            (value.len() * 2) as u32,
        )
    } else {
        let result = RegDeleteValueW(key, wide(STARTUP_NAME).as_ptr());
        if result == ERROR_FILE_NOT_FOUND {
            ERROR_SUCCESS
        } else {
            result
        }
    };
    RegCloseKey(key);
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("Cannot change startup registration ({status})."))
    }
}

pub unsafe fn pick_import(hwnd: HWND) -> Result<Option<PathBuf>, String> {
    let mut buffer = vec![0u16; 32768];
    let filter = wide("JSON settings\0*.json\0All files\0*.*\0");
    let mut dialog: OPENFILENAMEW = zeroed();
    dialog.lStructSize = size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = hwnd;
    dialog.lpstrFile = buffer.as_mut_ptr();
    dialog.nMaxFile = buffer.len() as u32;
    dialog.lpstrFilter = filter.as_ptr();
    // The title's storage must live through the modal call.
    let title = wide("Import Flow or WPF Throw preferences");
    dialog.lpstrTitle = title.as_ptr();
    dialog.Flags = OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR;
    if GetOpenFileNameW(&mut dialog) == 0 {
        let error = CommDlgExtendedError();
        return if error == 0 {
            Ok(None)
        } else {
            Err(format!("Cannot open file picker ({error})."))
        };
    }
    use std::os::windows::ffi::OsStringExt;
    let length = buffer.iter().position(|&v| v == 0).unwrap_or(buffer.len());
    Ok(Some(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length],
    ))))
}

pub unsafe fn message(hwnd: HWND, text: &str, error: bool) {
    MessageBoxW(
        hwnd,
        wide(text).as_ptr(),
        wide("Throw Launcher • Rust").as_ptr(),
        MB_OK
            | if error {
                MB_ICONERROR
            } else {
                MB_ICONINFORMATION
            },
    );
}

#[cfg(test)]
mod browser_tests {
    use super::*;

    #[test]
    fn direct_links_reject_non_urls_before_any_browser_is_started() {
        for browser in [Browser::WindowsDefault, Browser::Chrome] {
            for url in [
                "file:///C:/Windows/notepad.exe",
                "javascript:alert(1)",
                "--user-data-dir=C:/unexpected",
                "https://example.com/\" --bad-flag",
                "https://example.com/\0bad",
            ] {
                assert!(unsafe { open_direct_url(null_mut(), url, browser) }.is_err());
            }
        }
    }

    #[test]
    fn chrome_candidates_must_be_existing_absolute_chrome_executables() {
        let root = std::env::temp_dir().join(format!("throw-chrome-path-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let chrome = root.join("CHROME.EXE");
        let other = root.join("other.exe");
        std::fs::write(&chrome, b"fixture only; never launched").unwrap();
        std::fs::write(&other, b"fixture only; never launched").unwrap();
        assert!(valid_chrome_path(&chrome));
        assert!(!valid_chrome_path(&other));
        assert!(!valid_chrome_path(Path::new("chrome.exe")));
        assert!(!valid_chrome_path(&root.join("missing/chrome.exe")));
        assert!(!valid_chrome_path(&root));
        std::fs::remove_dir_all(root).unwrap();
    }
}
