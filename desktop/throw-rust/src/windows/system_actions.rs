//! Native command dispatch. Inputs are enums from a closed catalog, never query strings.
use super::platform::{open, wide};
use std::{
    ffi::OsStr,
    os::windows::{ffi::OsStringExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
    ptr::null,
};
use throw_launcher::commands::{SystemCommand, UtilityCommand};
use windows_sys::Win32::{Foundation::HWND, UI::Shell::SHEmptyRecycleBinW};

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemDirectoryW(buffer: *mut u16, size: u32) -> u32;
}
#[link(name = "user32")]
extern "system" {
    fn LockWorkStation() -> i32;
}
#[link(name = "powrprof")]
extern "system" {
    fn SetSuspendState(hibernate: u8, force: u8, disable_wake: u8) -> u8;
}

fn system_directory() -> Result<PathBuf, String> {
    let mut buffer = [0u16; 32768];
    let length = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if length == 0 || length >= buffer.len() {
        return Err("Cannot locate the trusted Windows system directory.".into());
    }
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..length],
    )))
}

pub unsafe fn execute(hwnd: HWND, command: SystemCommand, confirmed: bool) -> Result<(), String> {
    command.authorize(confirmed)?;
    if let Some(args) = command.shutdown_args() {
        let output = Command::new(system_directory()?.join("shutdown.exe"))
            .args(args)
            .creation_flags(0x08000000)
            .output()
            .map_err(|e| format!("Cannot request this Windows action: {e}"))?;
        return if output.status.success() {
            Ok(())
        } else {
            Err(format!(
                "Windows rejected this action ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ))
        };
    }
    let ok = match command {
        SystemCommand::Lock => LockWorkStation() != 0,
        SystemCommand::Sleep => SetSuspendState(0, 0, 0) != 0,
        SystemCommand::Hibernate => SetSuspendState(1, 0, 0) != 0,
        SystemCommand::EmptyRecycleBin => {
            let status = SHEmptyRecycleBinW(hwnd, null(), 1);
            if status < 0 {
                return Err(format!(
                    "Windows could not empty the Recycle Bin ({status:#x})."
                ));
            }
            true
        }
        _ => return Err("Unsupported system action.".into()),
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "Windows could not complete this action. It may be unavailable or restricted: {}",
            std::io::Error::last_os_error()
        ))
    }
}

pub unsafe fn utility(hwnd: HWND, command: UtilityCommand, root: &Path) -> Result<(), String> {
    use UtilityCommand::*;
    match command {
        WindowsSettings(page) => open(hwnd, OsStr::new(page.uri())),
        RecycleBin => open(hwnd, OsStr::new("shell:RecycleBinFolder")),
        ProfileFolder => open(hwnd, root.as_os_str()),
        ControlPanel | TaskManager | IndexOptions | CommandPrompt | PowerShell => {
            let (file, args): (&str, &[&str]) = match command {
                ControlPanel => ("control.exe", &[]),
                TaskManager => ("Taskmgr.exe", &[]),
                IndexOptions => ("control.exe", &["/name", "Microsoft.IndexingOptions"]),
                CommandPrompt => ("cmd.exe", &[]),
                PowerShell => ("WindowsPowerShell\\v1.0\\powershell.exe", &["-NoProfile"]),
                _ => unreachable!(),
            };
            // Absolute trusted executable + separate constant arguments. No PATH
            // resolution, shell concatenation, user query, or hidden scripts.
            Command::new(system_directory()?.join(file))
                .args(args)
                .creation_flags(if matches!(command, CommandPrompt | PowerShell) {
                    0x00000010
                } else {
                    0
                })
                .current_dir(root)
                .spawn()
                .map(|_| ())
                .map_err(|e| format!("Cannot open this Windows tool: {e}"))
        }
        _ => Err("This Throw action must be handled by the launcher.".into()),
    }
}

pub unsafe fn confirm(hwnd: HWND, command: SystemCommand) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match command.confirmation() {
        None => true,
        Some(text) => {
            MessageBoxW(
                hwnd,
                wide(text).as_ptr(),
                wide("Confirm system action — Throw").as_ptr(),
                MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
            ) == IDYES
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancelled_system_actions_never_reach_windows() {
        for row in throw_launcher::commands::search("") {
            if let throw_launcher::search::Action::System(command) = row.action {
                if command.confirmation().is_some() {
                    assert!(unsafe { execute(std::ptr::null_mut(), command, false) }.is_err());
                }
            }
        }
    }
    #[test]
    fn trusted_windows_tools_exist() {
        let root = system_directory().unwrap();
        assert!(root.is_absolute());
        assert!(root.join("shutdown.exe").is_file());
        assert!(root.join("control.exe").is_file());
    }
}
