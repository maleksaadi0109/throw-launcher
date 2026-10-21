//! Bounded directory names/paths only. Never read file contents or follow links.
use crate::config::{atomic_write, read_bounded};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashSet, VecDeque},
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub const LITE_LIMIT: usize = 10_000;
pub const STANDARD_LIMIT: usize = 30_000;
const CACHE_BYTES: u64 = 6 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Folder {
    pub name: String,
    pub path: PathBuf,
}

impl Folder {
    pub fn new(path: PathBuf) -> Option<Self> {
        let name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned();
        let folder = Self { name, path };
        folder.valid().then_some(folder)
    }
    pub fn valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= 512
            && !self.name.contains('\0')
            && self.path.is_absolute()
            && self.path.as_os_str().len() <= 4096
            && !self.path.to_string_lossy().contains('\0')
            && local_path(&self.path)
    }
}

fn local_path(path: &Path) -> bool {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        let b = value.as_bytes();
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && [b'\\', b'/'].contains(&b[2])
    }
    #[cfg(not(windows))]
    {
        path.is_absolute()
    }
}

pub struct Scan {
    pub entries: Vec<Folder>,
    pub limited: bool,
    pub unreadable: usize,
}

fn descend(path: &Path, metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Reparse points, hidden and system directories: no junction loops,
        // cloud hydration, OS trees or application caches.
        if metadata.file_attributes() & (0x400 | 0x2 | 0x4) != 0 {
            return false;
        }
    }
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    !name.starts_with('.')
        && ![
            "windows",
            "program files",
            "program files (x86)",
            "programdata",
            "appdata",
            "users",
            "node_modules",
            "target",
            "vendor",
            "$recycle.bin",
            "system volume information",
            "cache",
            "caches",
        ]
        .contains(&name.as_str())
}

pub fn scan(roots: &[PathBuf], lite: bool) -> Scan {
    scan_with_budget(
        roots,
        if lite { LITE_LIMIT } else { STANDARD_LIMIT },
        if lite { 60_000 } else { 180_000 },
        Duration::from_secs(if lite { 8 } else { 15 }),
    )
}

fn scan_with_budget(roots: &[PathBuf], limit: usize, visits: usize, duration: Duration) -> Scan {
    let mut result = Scan {
        entries: vec![],
        limited: false,
        unreadable: 0,
    };
    let mut queue = VecDeque::new();
    let mut seen = HashSet::new();
    let start = Instant::now();
    let mut visited = 0;
    let byte_limit = if limit <= LITE_LIMIT {
        2 * 1024 * 1024
    } else {
        4 * 1024 * 1024
    };
    let queue_limit = byte_limit / 2;
    let mut stored_bytes = 0;
    let mut queued_bytes = 0;
    for root in roots {
        if Folder::new(root.clone()).is_some()
            && root.is_dir()
            && queued_bytes + root.as_os_str().len() <= queue_limit
        {
            queued_bytes += root.as_os_str().len();
            queue.push_back((root.clone(), 0, if root.parent().is_none() { 3 } else { 8 }));
        }
    }
    while let Some((path, depth, max_depth)) = queue.pop_front() {
        queued_bytes -= path.as_os_str().len();
        if result.entries.len() >= limit || visited >= visits || start.elapsed() >= duration {
            result.limited = true;
            break;
        }
        if !seen.insert(path.to_string_lossy().to_lowercase()) {
            continue;
        }
        if let Some(folder) = Folder::new(path.clone()) {
            stored_bytes += folder.name.len() + folder.path.as_os_str().len();
            if stored_bytes > byte_limit {
                result.limited = true;
                break;
            }
            result.entries.push(folder);
        }
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            result.unreadable += 1;
            continue;
        };
        if depth >= max_depth
            || (!descend(&path, &metadata) && !(depth == 0 && path.parent().is_none()))
        {
            continue;
        }
        let children = match fs::read_dir(&path) {
            Ok(children) => children,
            Err(_) => {
                result.unreadable += 1;
                continue;
            }
        };
        for child in children {
            visited += 1;
            if visited >= visits || start.elapsed() >= duration {
                result.limited = true;
                break;
            }
            let Ok(child) = child else {
                result.unreadable += 1;
                continue;
            };
            let Ok(kind) = child.file_type() else {
                result.unreadable += 1;
                continue;
            };
            if kind.is_dir() && !kind.is_symlink() {
                let path = child.path();
                if queue.len() + result.entries.len() < limit
                    && queued_bytes + path.as_os_str().len() <= queue_limit
                {
                    queued_bytes += path.as_os_str().len();
                    queue.push_back((path, depth + 1, max_depth));
                } else {
                    result.limited = true;
                }
            }
        }
    }
    result
}

pub fn load(path: &Path, lite: bool) -> Result<Vec<Folder>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let bytes = read_bounded(path, CACHE_BYTES).map_err(|e| e.to_string())?;
    let mut entries: Vec<Folder> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if entries.len() > STANDARD_LIMIT || entries.iter().any(|e| !e.valid()) {
        return Err("Invalid folder cache; rebuild the search index.".into());
    }
    entries.truncate(if lite { LITE_LIMIT } else { STANDARD_LIMIT });
    Ok(entries)
}

pub fn save(path: &Path, entries: &[Folder]) -> Result<(), String> {
    let bytes = serde_json::to_vec(entries).map_err(|e| e.to_string())?;
    if entries.len() > STANDARD_LIMIT || bytes.len() as u64 > CACHE_BYTES {
        return Err("Folder index exceeds its bounded cache size.".into());
    }
    atomic_write(path, &bytes).map_err(|e| e.to_string())
}

/// Explicit local paths only: never turn arbitrary queries or UNC paths into IO.
pub fn browse(input: &str, limit: usize) -> Option<Vec<Folder>> {
    #[cfg(windows)]
    let is_path = {
        let b = input.as_bytes();
        b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && [b'\\', b'/'].contains(&b[2])
    };
    #[cfg(not(windows))]
    let is_path = input.starts_with('/');
    if !is_path || input.contains('\0') || input.len() > 512 {
        return None;
    }
    let path = PathBuf::from(input.replace('/', std::path::MAIN_SEPARATOR_STR));
    let (parent, filter) = if path.is_dir() {
        (path.clone(), String::new())
    } else {
        (
            path.parent()?.to_path_buf(),
            path.file_name()?.to_string_lossy().to_lowercase(),
        )
    };
    let mut found = vec![];
    if filter.is_empty() {
        if let Some(folder) = Folder::new(path) {
            found.push(folder);
        }
    }
    if let Ok(children) = fs::read_dir(parent) {
        for child in children.take(2048).flatten() {
            if child.file_type().is_ok_and(|t| t.is_dir())
                && child
                    .file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&filter)
            {
                if let Some(folder) = Folder::new(child.path()) {
                    found.push(folder);
                }
            }
        }
    }
    // An explicitly typed existing folder stays first: Enter opens that folder,
    // not an alphabetically earlier child (or a child replacing it at the cap).
    let start = usize::from(filter.is_empty() && !found.is_empty());
    found[start..].sort_by_key(|f| f.name.to_lowercase());
    found.truncate(limit.min(100));
    Some(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_scan_is_bounded_and_skips_cache_descent() {
        let root = std::env::temp_dir().join(format!("ThrowFolders-{}", std::process::id()));
        fs::create_dir_all(root.join("Projects/Example")).unwrap();
        fs::create_dir_all(root.join("node_modules/NotIndexed")).unwrap();
        fs::write(root.join("not-a-folder.txt"), "never read").unwrap();
        let result = scan_with_budget(
            std::slice::from_ref(&root),
            100,
            1000,
            Duration::from_secs(2),
        );
        assert!(result.entries.iter().any(|e| e.name == "Example"));
        assert!(!result
            .entries
            .iter()
            .any(|e| e.name == "NotIndexed" || e.name == "not-a-folder.txt"));
        let limited =
            scan_with_budget(std::slice::from_ref(&root), 2, 1000, Duration::from_secs(2));
        assert!(limited.limited);
        assert!(limited.entries.len() <= 2);
        let cache = root.join("folders.json");
        save(&cache, &result.entries).unwrap();
        assert_eq!(load(&cache, true).unwrap().len(), result.entries.len());
        let browse = browse(root.to_str().unwrap(), 50).unwrap();
        assert_eq!(browse[0].path, root);
        assert!(browse.iter().any(|e| e.name == "Projects"));
        assert!(!browse.iter().any(|e| e.name == "not-a-folder.txt"));
        assert!(Folder::new(PathBuf::from("relative")).is_none());
        fs::remove_dir_all(root).unwrap();
    }
}
