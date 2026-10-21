use crate::{calculator, config::Settings, index::Entry};

pub const MAX_QUERY: usize = 512;

#[derive(Clone, Debug)]
pub enum Action {
    App(usize),
    Url(String),
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
    let input = input.trim();
    if input.len() > MAX_QUERY || input.contains('\0') {
        return vec![];
    }
    let mut rows = Vec::new();
    if input.starts_with('>') {
        let filter = input.trim_start_matches('>').trim().to_lowercase();
        for (title, detail, action) in [
            (
                "Settings",
                "Appearance, shortcut, privacy and resources",
                Action::Settings,
            ),
            (
                "Rebuild app index",
                "Refresh Start menu and Desktop shortcuts",
                Action::Reindex,
            ),
            ("Quit Throw", "Exit the Rust launcher", Action::Quit),
        ] {
            if title.to_lowercase().contains(&filter) {
                rows.push(ResultRow {
                    title: title.into(),
                    detail: detail.into(),
                    action,
                });
            }
        }
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
    if settings.web_search && input.starts_with('?') {
        let terms = input.trim_start_matches('?').trim();
        if !terms.is_empty() {
            rows.push(ResultRow {
                title: format!("Search the web for {terms}"),
                detail: "Opens DuckDuckGo in your default browser".into(),
                action: Action::Url(format!(
                    "https://duckduckgo.com/?q={}",
                    percent_encode(terms)
                )),
            });
        }
        return rows;
    }
    if let Some(url) = safe_url(input) {
        rows.push(ResultRow {
            title: url.clone(),
            detail: "Open in default browser".into(),
            action: Action::Url(url),
        });
        return rows;
    }
    let needle = input.to_lowercase();
    // Keep only top-k, not a second allocation proportional to the full app index.
    let mut best: Vec<(i64, usize)> = Vec::with_capacity(settings.max_results + 1);
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
        let position = best.partition_point(|&(s, _)| s >= score);
        if position < settings.max_results {
            best.insert(position, (score, i));
            best.truncate(settings.max_results);
        }
    }
    for (_, i) in best {
        rows.push(ResultRow {
            title: entries[i].name.clone(),
            detail: entries[i].path.to_string_lossy().into_owned(),
            action: Action::App(i),
        });
    }
    rows
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

/// Explicit HTTP(S) only. No file:, javascript:, shell: or executable protocols.
pub fn safe_url(input: &str) -> Option<String> {
    if input.len() > MAX_QUERY || input.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return None;
    }
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    if authority.is_empty() || authority.contains(['@', '\\', '"', '<', '>']) {
        return None;
    }
    Some(input.into())
}

pub fn percent_encode(input: &str) -> String {
    let mut out = String::new();
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(matches!(&query("= 2+3", &[], &s)[0].action, Action::Copy(v) if v == "5"));
        assert!(query("= 1/0", &[], &s).is_empty());
        assert_eq!(query("?", &[], &s).len(), 0);
        assert!(query(&"x".repeat(513), &[], &s).is_empty());
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
}
