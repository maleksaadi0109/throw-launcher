//! Small, demand-loaded Shell icon cache. Shell work never runs in WM_PAINT.
use super::platform::{wide, ShellApartment};
use std::{
    collections::{HashSet, VecDeque},
    mem::{size_of, zeroed},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc,
    },
};
use windows_sys::Win32::{
    Foundation::HWND,
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub const ICONS_READY: u32 = WM_APP + 3;
const BATCH: usize = 12;

/// Owns a Shell-created icon. usize allows moving ownership across the worker
/// boundary; no thread uses the handle after transferring ownership.
struct OwnedIcon(usize);
impl Drop for OwnedIcon {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe {
                DestroyIcon(self.0 as HICON);
            }
        }
    }
}

fn extract(path: &Path) -> Option<OwnedIcon> {
    // Shell APIs are stricter about separators than ordinary Win32 file APIs.
    let normalized: PathBuf = path.components().collect();
    let mut info: SHFILEINFOW = unsafe { zeroed() };
    let result = unsafe {
        SHGetFileInfoW(
            wide(normalized).as_ptr(),
            0,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        )
    };
    if result != 0 && !info.hIcon.is_null() {
        Some(OwnedIcon(info.hIcon as usize))
    } else {
        if !info.hIcon.is_null() {
            unsafe {
                DestroyIcon(info.hIcon);
            }
        }
        None
    }
}

struct Store {
    entries: VecDeque<(PathBuf, Option<OwnedIcon>)>,
    capacity: usize,
}
impl Store {
    fn contains(&self, path: &Path) -> bool {
        self.entries.iter().any(|(p, _)| p == path)
    }
    fn insert(&mut self, path: PathBuf, icon: Option<OwnedIcon>) {
        if let Some(at) = self.entries.iter().position(|(p, _)| p == &path) {
            self.entries.remove(at); // Drop replaced handles, including failed lookups.
        }
        self.entries.push_back((path, icon));
        while self.entries.len() > self.capacity {
            self.entries.pop_front();
        }
    }
}

pub struct IconCache {
    store: Store,
    pending: HashSet<PathBuf>,
    requests: SyncSender<Vec<PathBuf>>,
    results: Receiver<(PathBuf, Option<OwnedIcon>)>,
    alive: Arc<AtomicBool>,
}
impl IconCache {
    pub fn new(hwnd: HWND, lite: bool) -> Self {
        let (requests, receive) = mpsc::sync_channel::<Vec<PathBuf>>(1);
        // Also bound results if Windows Shell finishes while the UI is in a dialog.
        let (send, results) = mpsc::sync_channel(BATCH * 2);
        let alive = Arc::new(AtomicBool::new(true));
        let active = alive.clone();
        let window = hwnd as usize;
        std::thread::spawn(move || {
            let apartment = unsafe { ShellApartment::initialize() };
            while let Ok(paths) = receive.recv() {
                if !active.load(Ordering::Acquire) {
                    break;
                }
                for path in paths {
                    if !active.load(Ordering::Acquire) {
                        break;
                    }
                    let icon = if apartment.is_ok() {
                        extract(&path)
                    } else {
                        None
                    };
                    if send.send((path, icon)).is_err() {
                        return;
                    }
                    if active.load(Ordering::Acquire) {
                        unsafe {
                            PostMessageW(window as HWND, ICONS_READY, 0, 0);
                        }
                    }
                }
            }
        });
        Self {
            store: Store {
                entries: VecDeque::new(),
                capacity: if lite { 48 } else { 96 },
            },
            pending: HashSet::new(),
            requests,
            results,
            alive,
        }
    }
    pub fn set_lite(&mut self, lite: bool) {
        self.store.capacity = if lite { 48 } else { 96 };
        while self.store.entries.len() > self.store.capacity {
            self.store.entries.pop_front();
        }
    }
    pub fn request(&mut self, paths: impl Iterator<Item = PathBuf>) {
        let mut unique = HashSet::new();
        let paths: Vec<_> = paths
            .filter(|p| {
                !self.store.contains(p) && !self.pending.contains(p) && unique.insert(p.clone())
            })
            .take(BATCH)
            .collect();
        if paths.is_empty() {
            return;
        }
        match self.requests.try_send(paths.clone()) {
            Ok(()) => self.pending.extend(paths),
            Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => (),
        }
    }
    pub fn drain(&mut self) -> bool {
        let mut changed = false;
        while let Ok((path, icon)) = self.results.try_recv() {
            self.pending.remove(&path);
            self.store.insert(path, icon);
            changed = true;
        }
        changed
    }
    pub fn get(&self, path: &Path) -> Option<HICON> {
        self.store
            .entries
            .iter()
            .find(|(p, _)| p == path)
            .and_then(|(_, icon)| icon.as_ref().map(|icon| icon.0 as HICON))
    }
    pub fn clear(&mut self) {
        self.store.entries.clear();
    }
    pub fn len(&self) -> usize {
        self.store.entries.len()
    }
}
impl Drop for IconCache {
    fn drop(&mut self) {
        self.alive.store(false, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shell_icons_are_owned_and_cache_is_bounded() {
        let _apartment = unsafe { ShellApartment::initialize() }.unwrap();
        let executable = std::env::current_exe().unwrap();
        let icon = extract(&executable).expect("Windows must return an executable icon");
        let mut store = Store {
            entries: VecDeque::new(),
            capacity: 2,
        };
        for i in 0..12 {
            let copy = unsafe { CopyIcon(icon.0 as HICON) };
            assert!(!copy.is_null());
            store.insert(
                PathBuf::from(format!("app-{i}.exe")),
                Some(OwnedIcon(copy as usize)),
            );
            assert!(store.entries.len() <= 2);
        }
        assert!(!store.contains(Path::new("app-0.exe")));
        assert!(store.contains(Path::new("app-11.exe")));
        store.insert("app-11.exe".into(), None);
        assert_eq!(store.entries.len(), 2);
        assert!(store.entries.back().unwrap().1.is_none());
    }
}
