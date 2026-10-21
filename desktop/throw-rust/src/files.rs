//! File search policy shared by the native UI, provider and tests.
use crate::search::{self, Action, ResultRow, MAX_QUERY};
use std::path::PathBuf;

pub const FILE_CANDIDATES: usize = 24;

#[derive(Clone, Debug)]
pub struct FileEntry {
    pub path: PathBuf,
}

impl FileEntry {
    pub fn valid(&self) -> bool {
        // Only local absolute drive paths from System.ItemPathDisplay, not URLs,
        // UNC shares, device paths or command lines.
        let path = self.path.to_string_lossy();
        let bytes = path.as_bytes();
        path.len() <= 32768
            && bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && bytes[2] == b'\\'
            && !path.chars().any(|c| c.is_control() || c == '"')
            && !path[3..].contains(':')
    }
}

/// Ordinary queries include files. `file:` reserves the list for files.
/// Special actions and empty/one-character searches never hit the disk index.
pub fn terms(input: &str) -> Option<&str> {
    let input = input.trim();
    if input.len() > MAX_QUERY
        || input.chars().any(char::is_control)
        || input.starts_with(['>', '=', '?'])
        || search::safe_url(input).is_some()
    {
        return None;
    }
    let value = input.strip_prefix("file:").unwrap_or(input).trim();
    (value.chars().count() >= 2).then_some(value)
}

pub fn sql(terms: &str) -> Result<String, String> {
    if terms.len() > MAX_QUERY || terms.chars().count() < 2 {
        return Err("Enter at least two characters for file search.".into());
    }
    // Reject LIKE metacharacters rather than accidentally expanding a query to
    // the entire catalog. Apostrophes are SQL literals, never query syntax.
    if terms.chars().any(|c| c.is_control() || "%_[]".contains(c)) {
        return Err("File search does not support %, _, [ or ] wildcards.".into());
    }
    let literal = terms.replace('\'', "''");
    Ok(format!(
        "SELECT TOP {FILE_CANDIDATES} System.ItemPathDisplay FROM SystemIndex \
         WHERE System.FileName LIKE '%{literal}%' AND System.ItemType <> 'Directory' \
         ORDER BY System.FileName ASC"
    ))
}

pub fn merge(rows: &mut Vec<ResultRow>, files: &[FileEntry], limit: usize, files_only: bool) {
    let limit = limit.min(12);
    if files_only {
        rows.clear();
    }
    let files: Vec<_> = files
        .iter()
        .take(FILE_CANDIDATES)
        .filter(|file| file.valid())
        .collect();
    if !files.is_empty() {
        // Files remain reachable even when app matches fill all available slots.
        rows.truncate(if files_only { 0 } else { limit / 2 });
        for file in files {
            if rows.len() >= limit {
                break;
            }
            if rows.iter().any(|row| {
                row.detail
                    .eq_ignore_ascii_case(&file.path.to_string_lossy())
            }) {
                continue;
            }
            let path = file.path.to_string_lossy();
            rows.push(ResultRow {
                title: format!("File • {}", path.rsplit('\\').next().unwrap_or(&path)),
                detail: path.into_owned(),
                action: Action::File(file.path.clone()),
            });
        }
    }
    rows.truncate(limit);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_and_sql_are_bounded_and_literal() {
        for input in [
            "",
            "a",
            "file: x",
            ">quit",
            "=2+3",
            "? hello",
            "https://example.com",
        ] {
            assert!(terms(input).is_none(), "{input}");
        }
        assert_eq!(terms("file: budget"), Some("budget"));
        assert_eq!(terms("budget"), Some("budget"));
        let query = sql("O'Brien'; DROP TABLE x --").unwrap();
        assert!(query.starts_with("SELECT TOP 24 "));
        assert!(query.contains("O''Brien''; DROP TABLE x --"));
        for input in ["%", "a_b", "a[b", "a]b", "aa\nbb"] {
            assert!(sql(input).is_err());
        }
    }

    #[test]
    fn files_cannot_be_urls_commands_or_device_paths() {
        for path in [
            r"\\server\share\x.txt",
            r"\\?\C:\x",
            "shell:AppsFolder",
            "cmd /c echo x",
            r"C:\x:stream",
            "C:\\x\n.txt",
        ] {
            assert!(!FileEntry { path: path.into() }.valid(), "{path}");
        }
        assert!(FileEntry {
            path: r"C:\Users\Me\budget.txt".into()
        }
        .valid());
    }

    #[test]
    fn combined_and_file_only_lists_are_bounded() {
        let files: Vec<_> = (0..100)
            .map(|n| FileEntry {
                path: format!(r"C:\Docs\{n}.txt").into(),
            })
            .collect();
        let app = ResultRow {
            title: "App".into(),
            detail: "app".into(),
            action: Action::App(0),
        };
        let mut rows = vec![app; 12];
        merge(&mut rows, &files, 12, false);
        assert_eq!(rows.len(), 12);
        assert!(matches!(rows[5].action, Action::App(_)));
        assert!(matches!(rows[6].action, Action::File(_)));
        merge(&mut rows, &files, 4, true);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|r| matches!(r.action, Action::File(_))));
    }
}
