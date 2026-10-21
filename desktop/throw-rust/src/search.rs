use crate::folders::{self, Folder};
pub use crate::web::{percent_encode, safe_url};
use crate::{calculator, config::Settings, index::Entry};
use std::path::PathBuf;

pub const MAX_QUERY: usize = 512;

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Internal native action-panel route; never generated from search text.
    Context(usize),
    System(crate::commands::SystemCommand),
    Utility(crate::commands::UtilityCommand),
    App(usize),
    Folder(PathBuf),
    File(PathBuf),
    Url(String),
    /// A URL typed directly, not a web-provider search or bookmark.
    DirectUrl(String),
    Copy(String),
    Settings,
    Reindex,
    Quit,
}

#[derive(Clone, Debug)]
pub struct ResultRow {
    pub title: String,
    pub detail: String,
    pub action: Action,
}

pub fn query(input: &str, entries: &[Entry], settings: &Settings) -> Vec<ResultRow> {
    query_all(input, entries, &[], settings)
}

pub fn query_all(
    input: &str,
    entries: &[Entry],
    folders: &[Folder],
    settings: &Settings,
) -> Vec<ResultRow> {
    query_sources(input, entries, folders, &[], settings)
}

pub fn query_sources(
    input: &str,
    entries: &[Entry],
    folders: &[Folder],
    bookmarks: &[crate::bookmarks::Bookmark],
    settings: &Settings,
) -> Vec<ResultRow> {
    let input = input.trim();
    if input.len() > MAX_QUERY || input.chars().any(char::is_control) {
        return vec![];
    }
    let mut rows = Vec::new();
    if input.starts_with("file:") {
        return rows;
    }
    if input.starts_with('>') {
        rows = crate::commands::search(input.trim_start_matches('>'));
        rows.truncate(settings.max_results);
        return rows;
    }
    if settings.calculator && input.starts_with('=') {
        return calculator::evaluate(input)
            .map(|n| {
                let value = n.to_string();
                vec![ResultRow {
                    title: value.clone(),
                    detail: "Calculator • Enter to copy".into(),
                    action: Action::Copy(value),
                }]
            })
            .unwrap_or_default();
    }
    if settings.web_search {
        if let Some((provider, terms)) = crate::web::match_provider(input, &settings.web_providers)
        {
            if let Some(url) = provider.destination(terms) {
                rows.push(ResultRow {
                    title: format!("Search {} for {terms}", provider.name),
                    detail: format!("Web · {} · {}", provider.keyword, provider.name),
                    action: Action::Url(url),
                });
            }
            return rows;
        }
    }
    if let Some(url) = safe_url(input) {
        rows.push(ResultRow {
            title: url.clone(),
            detail: match settings.direct_link_browser {
                crate::config::Browser::WindowsDefault => "Open in Windows default browser",
                crate::config::Browser::Chrome => "Open in Google Chrome",
            }
            .into(),
            action: Action::DirectUrl(url),
        });
        return rows;
    }
    if let Some(found) = folders::browse(input, settings.max_results) {
        return found
            .into_iter()
            .map(|f| ResultRow {
                title: f.name,
                detail: format!("Folder · {}", f.path.display()),
                action: Action::Folder(f.path),
            })
            .collect();
    }
    let needle = input.to_lowercase();
    // Keep only top-k, not a second allocation proportional to the full app index.
    let mut best: Vec<(i64, u8, usize)> = Vec::with_capacity(settings.max_results + 1);
    for (i, entry) in entries.iter().enumerate() {
        let name = entry.name.to_lowercase();
        let Some(mut score) = score(&needle, &name) else {
            continue;
        };
        score += settings
            .history
            .get(entry.path.to_string_lossy().as_ref())
            .copied()
            .unwrap_or(0)
            .min(25) as i64;
        let position = best.partition_point(|&(s, _, _)| s >= score);
        if position < settings.max_results {
            best.insert(position, (score, 0, i));
            best.truncate(settings.max_results);
        }
    }
    if !needle.is_empty() {
        for (i, folder) in folders.iter().enumerate() {
            let Some(score) = score(&needle, &folder.name.to_lowercase()) else {
                continue;
            };
            let position = best.partition_point(|&(s, _, _)| s >= score);
            if position < settings.max_results {
                best.insert(position, (score, 1, i));
                best.truncate(settings.max_results);
            }
        }
    }
    if settings.bookmarks && !needle.is_empty() {
        for (i, bookmark) in bookmarks
            .iter()
            .take(crate::bookmarks::MAX_BOOKMARKS)
            .enumerate()
        {
            let Some(score) = score(&needle, &bookmark.title.to_lowercase())
                .or_else(|| score(&needle, &bookmark.url.to_lowercase()).map(|s| s - 100))
            else {
                continue;
            };
            if safe_url(&bookmark.url).is_none() {
                continue;
            }
            let position = best.partition_point(|&(s, _, _)| s >= score);
            if position < settings.max_results {
                best.insert(position, (score, 2, i));
                best.truncate(settings.max_results);
            }
        }
    }
    for (_, kind, i) in best {
        rows.push(match kind {
            1 => folder_row(&folders[i]),
            2 => ResultRow {
                title: bookmarks[i].title.clone(),
                detail: format!("Bookmark · {} · {}", bookmarks[i].source, bookmarks[i].url),
                action: Action::Url(bookmarks[i].url.clone()),
            },
            _ => ResultRow {
                title: entries[i].name.clone(),
                detail: entries[i].path.to_string_lossy().into_owned(),
                action: Action::App(i),
            },
        });
    }
    // Mix the known/root folders into the empty-query launch list, rather than
    // hiding them behind all installed apps. Search still ranks by actual match.
    if needle.is_empty() {
        for (i, folder) in folders.iter().take(6).enumerate() {
            let position = (2 + i * 3).min(rows.len());
            rows.insert(position, folder_row(folder));
        }
        rows.truncate(settings.max_results);
    }
    // Commands are discoverable without a special prefix, but never offered
    // on the empty launch screen. No user text becomes an executable argument.
    if needle.chars().count() >= 2 {
        let mut commands = crate::commands::search(&needle);
        // Throw preferences live in the Settings button and explicit > palette.
        // Ordinary "settings" searches should surface Windows settings instead.
        commands.retain(|row| !matches!(row.action, Action::Settings));
        commands.append(&mut rows);
        commands.truncate(settings.max_results);
        rows = commands;
    }
    rows
}

fn folder_row(folder: &Folder) -> ResultRow {
    ResultRow {
        title: folder.name.clone(),
        detail: format!("Folder · {}", folder.path.display()),
        action: Action::Folder(folder.path.clone()),
    }
}

fn score(needle: &str, name: &str) -> Option<i64> {
    if needle.is_empty() {
        return Some(0);
    }
    if name == needle {
        return Some(1000);
    }
    if name.starts_with(needle) {
        return Some(800 - name.len().min(100) as i64);
    }
    if let Some(position) = name.find(needle) {
        return Some(600 - position.min(100) as i64);
    }
    let mut chars = name.chars();
    for letter in needle.chars().filter(|c| !c.is_whitespace()) {
        chars.find(|&c| c == letter)?;
    }
    Some(200 - name.len().min(100) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn browser_preference_only_changes_direct_links() {
        let mut settings = Settings {
            bookmarks: true,
            ..Settings::default()
        };
        let bookmark = crate::bookmarks::Bookmark {
            title: "Handbook".into(),
            url: "https://example.com/bookmark".into(),
            source: "Chrome".into(),
        };
        for (browser, detail) in [
            (
                crate::config::Browser::WindowsDefault,
                "Open in Windows default browser",
            ),
            (crate::config::Browser::Chrome, "Open in Google Chrome"),
        ] {
            settings.direct_link_browser = browser;
            for url in ["https://example.com/a?q=a&b=%22", "http://example.com/"] {
                let rows = query(url, &[], &settings);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].action, Action::DirectUrl(url.into()));
                assert_eq!(rows[0].detail, detail);
            }
            assert!(matches!(&query("?rust lang", &[], &settings)[0].action,
                Action::Url(url) if url == "https://www.google.com/search?q=rust%20lang"));
            assert_eq!(
                query_sources(
                    "handbook",
                    &[],
                    &[],
                    std::slice::from_ref(&bookmark),
                    &settings
                )[0]
                .action,
                Action::Url(bookmark.url.clone())
            );
            settings.web_search = false;
            assert!(matches!(
                query("https://example.com", &[], &settings)[0].action,
                Action::DirectUrl(_)
            ));
            assert!(query("?rust lang", &[], &settings).is_empty());
            assert!(query("javascript:alert(1)", &[], &settings).is_empty());
            assert!(matches!(
                query("= 2+3", &[], &settings)[0].action,
                Action::Copy(_)
            ));
            settings.web_search = true;
        }
    }

    #[test]
    fn exact_prefix_and_fuzzy_ranking() {
        let entries: Vec<_> = ["Visual Studio Code", "Code", "Codec", "Chrome"]
            .iter()
            .map(|name| Entry {
                name: name.to_string(),
                path: std::env::temp_dir().join(format!("{name}.lnk")),
            })
            .collect();
        let s = Settings::default();
        let rows = query("code", &entries, &s);
        assert_eq!(rows[0].title, "Code");
        assert_eq!(rows[1].title, "Codec");
        assert_eq!(query("vsc", &entries, &s)[0].title, "Visual Studio Code");
        assert!(query("not here", &entries, &s).is_empty());
    }
    #[test]
    fn commands_and_actions() {
        let s = Settings::default();
        assert!(matches!(query(">quit", &[], &s)[0].action, Action::Quit));
        for text in ["settings", "setting", "seting"] {
            assert!(!query(text, &[], &s)
                .iter()
                .any(|r| matches!(r.action, Action::Settings)));
        }
        assert!(query("settings", &[], &s).iter().any(|r| matches!(
            r.action,
            Action::Utility(crate::commands::UtilityCommand::WindowsSettings(_))
        )));
        assert!(query(">settings", &[], &s)
            .iter()
            .any(|r| matches!(r.action, Action::Settings)));
        assert!(matches!(&query("= 2+3", &[], &s)[0].action, Action::Copy(v) if v == "5"));
        assert!(query("= 1/0", &[], &s).is_empty());
        assert_eq!(query("?", &[], &s).len(), 0);
        assert!(query(&"x".repeat(513), &[], &s).is_empty());
        assert!(matches!(
            query("reboot", &[], &s)[0].action,
            Action::System(crate::commands::SystemCommand::Restart)
        ));
        assert_eq!(query(">", &[], &s).len(), crate::commands::search("").len());
        let limited = Settings {
            max_results: 4,
            ..s
        };
        assert_eq!(query(">", &[], &limited).len(), 4);
        assert!(query(">restart && calc.exe", &[], &limited).is_empty());
    }
    #[test]
    fn urls_reject_unsafe_protocols_and_encode_search_terms() {
        for url in [
            "file:///C:/evil.exe",
            "javascript:alert(1)",
            "shell:appsFolder",
            "https://",
            "https://a@evil",
            "https://a\nb",
        ] {
            assert!(safe_url(url).is_none(), "{url}");
        }
        assert!(safe_url("https://example.com/path?q=a").is_some());
        assert_eq!(percent_encode("a&b é"), "a%26b%20%C3%A9");
    }
    #[test]
    fn bookmark_and_provider_actions_are_real_optional_and_bounded() {
        let bookmarks = [
            crate::bookmarks::Bookmark {
                title: "Rust handbook".into(),
                url: "https://rust-lang.org/learn".into(),
                source: "Chrome".into(),
            },
            crate::bookmarks::Bookmark {
                title: "Rust unsafe".into(),
                url: "file:///evil.exe".into(),
                source: "Untrusted".into(),
            },
        ];
        let mut settings = Settings::default();
        assert!(query_sources("handbook", &[], &[], &bookmarks, &settings).is_empty());
        settings.bookmarks = true;
        let rows = query_sources("handbook", &[], &[], &bookmarks, &settings);
        assert!(
            matches!(&rows[0].action, Action::Url(url) if url == "https://rust-lang.org/learn")
        );
        assert_eq!(
            query_sources("rust", &[], &[], &bookmarks, &settings).len(),
            1
        );
        assert!(query_sources("", &[], &[], &bookmarks, &settings).is_empty());
        settings.web_providers.push(crate::web::Provider {
            name: "Example".into(),
            keyword: "ex".into(),
            template: "https://example.com/search?q={query}".into(),
            enabled: true,
        });
        assert!(
            matches!(&query("EX a&b é", &[], &settings)[0].action, Action::Url(url) if url == "https://example.com/search?q=a%26b%20%C3%A9")
        );
        assert!(query("ex", &[], &settings).is_empty());
        settings.web_providers[1].enabled = false;
        assert!(query("ex something", &[], &settings).is_empty());
        settings.web_search = false;
        assert!(query("?something", &[], &settings).is_empty());
        assert!(query("= 2 + 3", &[], &settings).len() == 1);
        settings.web_search = true;
        settings.web_providers.clear();
        assert!(query("?something", &[], &settings).is_empty());
        assert!(query("https://example.com", &[], &settings).len() == 1);
        let lots = vec![bookmarks[0].clone(); 200];
        assert_eq!(
            query_sources("handbook", &[], &[], &lots, &settings).len(),
            settings.max_results
        );
    }
    #[test]
    fn result_count_and_history_influence() {
        let mut s = Settings {
            max_results: 4,
            ..Settings::default()
        };
        let entries: Vec<_> = (0..100)
            .map(|i| Entry {
                name: format!("App {i}"),
                path: std::env::temp_dir().join(format!("App{i}.lnk")),
            })
            .collect();
        s.record_launch(entries[88].path.to_string_lossy().into_owned());
        let rows = query("", &entries, &s);
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0].title, "App 88");
    }

    #[test]
    fn folders_mix_with_apps_and_rank_by_name_without_limiting_viewport() {
        let settings = Settings::default();
        let apps: Vec<_> = (0..80)
            .map(|i| Entry {
                name: format!("App {i}"),
                path: std::env::temp_dir().join(format!("App{i}.exe")),
            })
            .collect();
        let folder = Folder::new(std::env::temp_dir().join("Projects")).unwrap();
        let folders = [folder];
        let rows = query_all("", &apps, &folders, &settings);
        assert_eq!(rows.len(), 50);
        assert!(matches!(rows[0].action, Action::App(_)));
        assert!(matches!(rows[2].action, Action::Folder(_)));
        let matching = query_all("projects", &apps, &folders, &settings);
        assert_eq!(matching[0].title, "Projects");
        assert!(matches!(matching[0].action, Action::Folder(_)));
        assert!(matches!(
            query_all("= 2+3", &apps, &folders, &settings)[0].action,
            Action::Copy(_)
        ));
    }
}
