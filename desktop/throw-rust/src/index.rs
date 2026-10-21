use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

pub const LITE_INDEX_LIMIT: usize = 5_000;
pub const STANDARD_INDEX_LIMIT: usize = 20_000;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
}

impl Entry {
    pub fn valid(&self) -> bool {
        !self.name.is_empty()
            && self.name.len() <= 512
            && !self.name.contains('\0')
            && self.path.is_absolute()
            && self.path.as_os_str().len() <= 4096
            && !self.path.to_string_lossy().contains('\0')
            && allowed_extension(&self.path)
    }
}

fn allowed_extension(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        ["lnk", "exe", "appref-ms"]
            .iter()
            .any(|v| e.eq_ignore_ascii_case(v))
    })
}

pub struct Scan {
    pub entries: Vec<Entry>,
    pub unreadable: usize,
    pub truncated: bool,
}

/// Only caller-selected app folders, never the whole disk. No symlinks or junctions.
pub fn scan(roots: &[PathBuf], limit: usize) -> Scan {
    let limit = limit.min(STANDARD_INDEX_LIMIT);
    let mut result = Scan {
        entries: Vec::new(),
        unreadable: 0,
        truncated: false,
    };
    let mut visited = 0usize;
    let mut seen = HashSet::new();
    let mut stack: Vec<_> = roots.iter().map(|p| (p.clone(), 0usize)).collect();
    while let Some((directory, depth)) = stack.pop() {
        if depth > 8 {
            result.truncated = true;
            continue;
        }
        let children = match fs::read_dir(&directory) {
            Ok(children) => children,
            Err(_) => {
                if directory.exists() {
                    result.unreadable += 1;
                }
                continue;
            }
        };
        for child in children {
            visited += 1;
            if visited > 60_000 || result.entries.len() >= limit {
                result.truncated = true;
                result.entries.sort_by_key(|e| e.name.to_lowercase());
                return result;
            }
            let Ok(child) = child else {
                result.unreadable += 1;
                continue;
            };
            let Ok(metadata) = child.metadata() else {
                result.unreadable += 1;
                continue;
            };
            let Ok(file_type) = child.file_type() else {
                continue;
            };
            if file_type.is_symlink() || is_reparse(&metadata) {
                continue;
            }
            let path = child.path();
            if file_type.is_dir() {
                stack.push((path, depth + 1));
                continue;
            }
            if !file_type.is_file() || !allowed_extension(&path) {
                continue;
            }
            let name = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let entry = Entry { name, path };
            if entry.valid() && seen.insert(entry.path.to_string_lossy().to_lowercase()) {
                result.entries.push(entry);
            }
        }
    }
    result.entries.sort_by_key(|e| e.name.to_lowercase());
    result
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}
#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scan_limits_extensions_and_duplicates() {
        let root = std::env::temp_dir().join(format!("throw-index-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        for name in [
            "Editor.lnk",
            "Other.exe",
            "Script.bat",
            "readme.txt",
            "Click.url",
        ] {
            fs::write(root.join(name), b"").unwrap();
        }
        let all = scan(&[root.clone(), root.clone()], 100);
        assert_eq!(all.entries.len(), 2);
        assert!(all.entries.iter().all(Entry::valid));
        let limited = scan(&[root.clone()], 1);
        assert_eq!(limited.entries.len(), 1);
        assert!(limited.truncated);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn cached_paths_must_be_absolute_and_allowlisted() {
        assert!(!Entry {
            name: "bad".into(),
            path: "cmd.bat".into()
        }
        .valid());
        assert!(!Entry {
            name: "".into(),
            path: std::env::temp_dir().join("app.exe")
        }
        .valid());
    }
}
