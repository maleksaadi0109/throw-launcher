//! Untrusted web-provider configuration. Never interpret a template as a command.
use serde::{Deserialize, Serialize};

pub const MAX_PROVIDERS: usize = 16;
pub const MAX_URL: usize = 4096;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Provider {
    pub name: String,
    pub keyword: String,
    pub template: String,
    pub enabled: bool,
}

impl Provider {
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("Provider name must contain 1–80 bytes of plain text.".into());
        }
        if self.keyword != "?"
            && (self.keyword.is_empty()
                || self.keyword.len() > 16
                || !self.keyword.bytes().all(|b| b.is_ascii_alphanumeric())
                || !self.keyword.as_bytes()[0].is_ascii_alphabetic())
        {
            return Err("Keyword must start with a letter and contain up to 16 ASCII letters/digits (or ?).".into());
        }
        let authority = self
            .template
            .split("://")
            .nth(1)
            .and_then(|s| s.split(['/', '?', '#']).next())
            .unwrap_or("");
        if self.template.len() > 2048
            || self.template.matches("{query}").count() != 1
            || authority.contains('{')
            || safe_url(&self.template.replace("{query}", "test")).is_none()
        {
            return Err("Use an HTTP(S) URL with exactly one {query} in its path, query or fragment, never its host.".into());
        }
        Ok(())
    }

    pub fn destination(&self, terms: &str) -> Option<String> {
        if !self.enabled
            || terms.is_empty()
            || terms.len() > crate::search::MAX_QUERY
            || self.validate().is_err()
        {
            return None;
        }
        safe_url(&self.template.replace("{query}", &percent_encode(terms)))
    }
}

pub fn defaults() -> Vec<Provider> {
    vec![Provider {
        name: "Google".into(),
        keyword: "?".into(),
        template: "https://www.google.com/search?q={query}".into(),
        enabled: true,
    }]
}

pub(crate) fn previous_defaults() -> Vec<Provider> {
    vec![Provider {
        name: "DuckDuckGo".into(),
        keyword: "?".into(),
        template: "https://duckduckgo.com/?q={query}".into(),
        enabled: true,
    }]
}

/// Share routing with file discovery so a keyword search cannot acquire late
/// file results that change what Enter opens.
pub fn match_provider<'a>(
    input: &'a str,
    providers: &'a [Provider],
) -> Option<(&'a Provider, &'a str)> {
    let input = input.trim();
    let (keyword, terms) = if let Some(terms) = input.strip_prefix('?') {
        ("?", terms.trim())
    } else {
        input
            .split_once(char::is_whitespace)
            .map(|(k, t)| (k, t.trim()))
            .unwrap_or((input, ""))
    };
    providers
        .iter()
        .find(|p| p.enabled && p.keyword.eq_ignore_ascii_case(keyword))
        .map(|p| (p, terms))
}

pub fn validate_providers(providers: &[Provider]) -> Result<(), String> {
    if providers.len() > MAX_PROVIDERS {
        return Err(format!(
            "At most {MAX_PROVIDERS} web providers are supported."
        ));
    }
    let mut keywords = std::collections::BTreeSet::new();
    for provider in providers {
        provider.validate()?;
        if !keywords.insert(provider.keyword.to_ascii_lowercase()) {
            return Err("Provider keywords must be unique (case-insensitive).".into());
        }
    }
    Ok(())
}

/// A deliberately strict HTTP(S) subset, including local hosts and IPv6.
/// Reject ambiguous Shell/browser parsing, credentials and encoded authorities.
pub fn safe_url(input: &str) -> Option<String> {
    if input.len() > MAX_URL
        || input.chars().any(|c| c.is_control() || c.is_whitespace())
        || input.contains(['\\', '"', '<', '>', '{', '}'])
    {
        return None;
    }
    let rest = input
        .strip_prefix("https://")
        .or_else(|| input.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let port = if authority.starts_with('[') {
        let end = authority.find(']')?;
        authority[1..end].parse::<std::net::Ipv6Addr>().ok()?;
        &authority[end + 1..]
    } else {
        let host = authority.split(':').next()?;
        if host.is_empty()
            || host.len() > 253
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
            || host
                .split('.')
                .any(|s| s.is_empty() || s.starts_with('-') || s.ends_with('-'))
        {
            return None;
        }
        &authority[host.len()..]
    };
    if !port.is_empty() {
        let digits = port.strip_prefix(':')?;
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.parse::<u16>().ok()? == 0
        {
            return None;
        }
    }
    let bytes = input.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'%'
            && (i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit())
        {
            return None;
        }
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
    fn strict_destinations() {
        for bad in [
            "https://",
            "javascript:alert(1)",
            "file:///a",
            "https://a@b",
            "https://%61",
            "https://a\\evil",
            "https://a/\\evil",
            "https://a:bad",
            "https://a:65536",
            "https://a/%x",
            "https://a\nb",
            "https://[invalid]",
            "https://a/{evil}",
        ] {
            assert!(safe_url(bad).is_none(), "{bad}");
        }
        for good in [
            "https://example.com/a?q=a%20b",
            "http://localhost:8080",
            "https://[::1]:443/a",
        ] {
            assert!(safe_url(good).is_some(), "{good}");
        }
    }
    #[test]
    fn templates_keywords_and_encoding() {
        let mut p = defaults().remove(0);
        assert!(p
            .destination(&"x".repeat(crate::search::MAX_QUERY + 1))
            .is_none());
        assert_eq!(
            p.destination("a&b é#?").unwrap(),
            "https://www.google.com/search?q=a%26b%20%C3%A9%23%3F"
        );
        p.enabled = false;
        assert!(p.destination("hello").is_none());
        for template in [
            "https://{query}.com",
            "https://a/{query}/{query}",
            "https://a/{other}/{query}",
            "javascript:{query}",
            "https://a/",
        ] {
            p.template = template.into();
            assert!(p.validate().is_err(), "{template}");
        }
        let mut p = defaults().remove(0);
        p.keyword = ">".into();
        assert!(p.validate().is_err());
        p.keyword = "Go".into();
        let mut other = p.clone();
        other.keyword = "go".into();
        assert!(validate_providers(&[p, other]).is_err());
        assert!(validate_providers(&vec![defaults()[0].clone(); MAX_PROVIDERS + 1]).is_err());
    }
}
