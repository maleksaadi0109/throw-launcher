//! Read-only, opt-in Chromium bookmark snapshots; no browser databases or runtimes.
use crate::web::safe_url;
use serde_json::Value;
use std::{
    collections::BTreeSet,
    io::{self, Read},
    path::{Path, PathBuf},
};

pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_TOTAL_BYTES: u64 = 8 * 1024 * 1024;
pub const MAX_FILES: usize = 16;
pub const MAX_BOOKMARKS: usize = 4000;
const MAX_NODES: usize = 12000;

#[derive(Clone, Debug, PartialEq)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
    pub source: String,
}

#[derive(Default)]
pub struct Scan {
    pub entries: Vec<Bookmark>,
    pub errors: Vec<String>,
    pub limited: bool,
    pub skipped: usize,
}

pub fn parse(bytes: &[u8], source: &str) -> Result<Scan, String> {
    if bytes.len() as u64 > MAX_FILE_BYTES || source.len() > 80 {
        return Err("Bookmark input exceeds safe limits.".into());
    }
    let value: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("Invalid bookmark JSON: {e}"))?;
    let roots = value
        .get("roots")
        .and_then(Value::as_object)
        .ok_or("Bookmark roots must be an object.")?;
    let mut scan = Scan::default();
    let mut nodes = 0;
    for node in roots.values() {
        visit(node, source, 0, &mut nodes, &mut scan)?;
    }
    Ok(scan)
}

fn visit(
    node: &Value,
    source: &str,
    depth: usize,
    nodes: &mut usize,
    scan: &mut Scan,
) -> Result<(), String> {
    *nodes += 1;
    if depth > 32 || *nodes > MAX_NODES || scan.entries.len() >= MAX_BOOKMARKS {
        scan.limited = true;
        return Ok(());
    }
    match node.get("type").and_then(Value::as_str) {
        Some("folder") => {
            let children = node
                .get("children")
                .and_then(Value::as_array)
                .ok_or("Bookmark folder children must be an array.")?;
            for child in children {
                visit(child, source, depth + 1, nodes, scan)?;
                if scan.limited {
                    break;
                }
            }
        }
        Some("url") => {
            let title = node
                .get("name")
                .and_then(Value::as_str)
                .ok_or("Bookmark name must be text.")?;
            let url = node
                .get("url")
                .and_then(Value::as_str)
                .ok_or("Bookmark URL must be text.")?;
            if title.len() > 512 || title.chars().any(char::is_control) || safe_url(url).is_none() {
                scan.skipped += 1;
            } else {
                scan.entries.push(Bookmark {
                    title: if title.trim().is_empty() {
                        url.into()
                    } else {
                        title.into()
                    },
                    url: url.into(),
                    source: source.into(),
                });
            }
        }
        _ => return Err("Unsupported bookmark node schema.".into()),
    }
    Ok(())
}

/// Fixed candidate paths: no recursive drive/profile discovery.
pub fn candidates(local: &Path) -> Vec<(PathBuf, String)> {
    let mut paths = Vec::new();
    for (dir, browser) in [
        ("Google/Chrome/User Data", "Chrome"),
        ("Microsoft/Edge/User Data", "Edge"),
        ("BraveSoftware/Brave-Browser/User Data", "Brave"),
        ("Chromium/User Data", "Chromium"),
    ] {
        for profile in
            std::iter::once("Default".into()).chain((1..=8).map(|i| format!("Profile {i}")))
        {
            paths.push((
                local.join(dir).join(&profile).join("Bookmarks"),
                format!("{browser} · {profile}"),
            ));
        }
    }
    paths
}

fn ordinary_path(base: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(base) else {
        return false;
    };
    let mut current = base.to_path_buf();
    for part in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(part) = part {
            current.push(part);
        }
        let Ok(meta) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        if meta.file_type().is_symlink()
            || (current == path && !meta.is_file())
            || (current != path && !meta.is_dir())
        {
            return false;
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {
                return false;
            }
        }
    }
    true
}

/// Charge bytes even on partial read errors or subsequent parse failures.
/// The extra growth-detection byte also comes from the shared snapshot budget.
fn read_snapshot(path: &Path, total: &mut u64) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let meta = file.metadata()?;
    let remaining = MAX_TOTAL_BYTES.saturating_sub(*total);
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES || meta.len() > remaining {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Bookmark file exceeds its file or remaining snapshot input budget.",
        ));
    }
    let expected = meta.len();
    let mut bytes = Vec::new();
    let result = file
        .take((expected + 1).min(remaining))
        .read_to_end(&mut bytes);
    *total += bytes.len() as u64;
    result?;
    if bytes.len() as u64 > expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Bookmark file grew during reading; refresh to retry.",
        ));
    }
    Ok(bytes)
}

pub fn discover(local: &Path, enabled: bool) -> Scan {
    discover_while(local, || enabled)
}

/// Permission is rechecked between candidate files, including before each read.
pub fn discover_while(local: &Path, mut permitted: impl FnMut() -> bool) -> Scan {
    let mut result = Scan::default();
    if !permitted() {
        return result;
    }
    let mut files = 0;
    let mut total = 0;
    let mut seen = BTreeSet::new();
    for (path, source) in candidates(local) {
        if !permitted() {
            return Scan::default();
        }
        // Missing profiles are normal; unreadable/redirected existing files are not.
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => {
                result.errors.push(format!("{source}: {e}"));
                continue;
            }
            Ok(_) => {}
        }
        if files >= MAX_FILES || total >= MAX_TOTAL_BYTES || result.entries.len() >= MAX_BOOKMARKS {
            result.limited = true;
            break;
        }
        files += 1;
        if !ordinary_path(local, &path) {
            result.errors.push(format!(
                "{source}: redirected or unreadable bookmark path skipped."
            ));
            continue;
        }
        if !permitted() {
            return Scan::default();
        }
        match read_snapshot(&path, &mut total)
            .map_err(|e| e.to_string())
            .and_then(|bytes| parse(&bytes, &source))
        {
            Ok(scan) => {
                result.limited |= scan.limited;
                result.skipped += scan.skipped;
                for entry in scan.entries {
                    if result.entries.len() >= MAX_BOOKMARKS {
                        result.limited = true;
                        break;
                    }
                    if seen.insert(entry.url.clone()) {
                        result.entries.push(entry);
                    }
                }
            }
            Err(error) => result.errors.push(format!("{source}: {error}")),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    pub const FIXTURE: &[u8] = br#"{"roots":{"bookmark_bar":{"type":"folder","children":[{"type":"url","name":"Rust handbook","url":"https://www.rust-lang.org/learn"},{"type":"url","name":"Unsafe","url":"javascript:alert(1)"}]}}}"#;
    #[test]
    fn nested_real_schema_and_invalid_input() {
        let scan = parse(FIXTURE, "Chrome").unwrap();
        assert_eq!(scan.entries.len(), 1);
        assert_eq!(scan.skipped, 1);
        for bytes in [
            b"[]".as_slice(),
            br#"{"roots":[]}"#,
            br#"{"roots":{"a":{"type":"folder","children":42}}}"#,
            br#"{"roots":{"a":{"type":"url","name":false,"url":"https://a"}}}"#,
        ] {
            assert!(parse(bytes, "test").is_err());
        }
        assert!(parse(&vec![b' '; MAX_FILE_BYTES as usize + 1], "test").is_err());
        let nodes: Vec<_> = (0..MAX_BOOKMARKS + 10)
            .map(|_| serde_json::json!({"type":"url","name":"A","url":"https://a"}))
            .collect();
        let big = serde_json::json!({"roots":{"bar":{"type":"folder","children":nodes}}});
        let scan = parse(&serde_json::to_vec(&big).unwrap(), "test").unwrap();
        assert_eq!(scan.entries.len(), MAX_BOOKMARKS);
        assert!(scan.limited);
    }
    #[test]
    fn opt_in_discovery_dedup_and_bounds() {
        let root = std::env::temp_dir().join(format!("throw-bookmarks-{}", std::process::id()));
        let paths = candidates(&root);
        for (path, _) in paths.iter().take(2) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, FIXTURE).unwrap();
        }
        assert!(discover(&root, false).entries.is_empty());
        let scan = discover(&root, true);
        assert_eq!(scan.entries.len(), 1);
        assert_eq!(scan.skipped, 2);
        std::fs::write(&paths[0].0, b"broken").unwrap();
        assert_eq!(discover(&root, true).errors.len(), 1);
        let mut checks = 0;
        let cancelled = discover_while(&root, || {
            checks += 1;
            checks < 3
        });
        assert!(cancelled.entries.is_empty() && cancelled.errors.is_empty());
        assert_eq!(checks, 3); // Permission withdrawn immediately before opening the file.
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn depth_file_count_and_shared_read_budget_are_bounded() {
        let mut node = serde_json::json!({"type":"url","name":"Leaf","url":"https://a"});
        for _ in 0..40 {
            node = serde_json::json!({"type":"folder","children":[node]});
        }
        let bytes = serde_json::to_vec(&serde_json::json!({"roots":{"bar":node}})).unwrap();
        assert!(parse(&bytes, "test").unwrap().limited);
        let root =
            std::env::temp_dir().join(format!("throw-bookmark-bounds-{}", std::process::id()));
        for (path, _) in candidates(&root).iter().take(MAX_FILES + 1) {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, FIXTURE).unwrap();
        }
        let scan = discover(&root, true);
        assert!(scan.limited);
        assert_eq!(scan.skipped, MAX_FILES);
        let path = root.join("budget-fixture");
        std::fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize]).unwrap();
        let mut total = 0;
        for _ in 0..4 {
            assert!(read_snapshot(&path, &mut total).is_ok());
        }
        assert_eq!(total, MAX_TOTAL_BYTES);
        assert!(read_snapshot(&path, &mut total).is_err());
        assert_eq!(total, MAX_TOTAL_BYTES);
        std::fs::write(&path, vec![b' '; MAX_FILE_BYTES as usize + 1]).unwrap();
        assert!(read_snapshot(&path, &mut 0).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn redirected_browser_files_are_not_read() {
        let root =
            std::env::temp_dir().join(format!("throw-bookmark-links-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("outside.json");
        std::fs::write(&target, FIXTURE).unwrap();
        let path = &candidates(&root)[0].0;
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&target, path).unwrap();
        let scan = discover(&root, true);
        assert!(scan.entries.is_empty());
        assert_eq!(scan.errors.len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }
}
