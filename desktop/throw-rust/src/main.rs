#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod windows;

fn main() {
    #[cfg(windows)]
    if let Err(error) = windows::run() {
        windows::show_error(&error);
        std::process::exit(1);
    }
    #[cfg(not(windows))]
    {
        eprintln!(
            "Throw Launcher is a native Windows app. Run cargo test here to test its engine."
        );
        std::process::exit(1);
    }
}
