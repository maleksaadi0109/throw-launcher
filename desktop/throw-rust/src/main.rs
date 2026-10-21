#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod windows;

fn main() {
    #[cfg(windows)]
    if let Err(error) = windows::run() {
        if std::env::args().any(|arg| arg == "--smoke-test" || arg == "--file-search-benchmark") {
            let _ = std::fs::write("smoke-error.txt", &error);
        } else {
            windows::show_error(&error);
        }
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
