//! Strict installer-only preference input. No paths or commands are accepted.
use crate::config::{Hotkey, Settings, Theme};
use std::path::Path;

pub struct Choices {
    theme: Theme,
    hotkey: Hotkey,
    lite: bool,
    hide_on_focus_loss: bool,
    history: bool,
}

impl Choices {
    pub fn parse(args: &[String]) -> Result<Self, String> {
        if args.len() != 5 {
            return Err("Installer setup requires exactly five preference choices.".into());
        }
        let mut values = std::collections::BTreeMap::new();
        for arg in args {
            let (key, value) = arg.split_once('=').ok_or("Invalid installer preference.")?;
            if ![
                "--theme",
                "--hotkey",
                "--lite",
                "--hide-on-focus-loss",
                "--history",
            ]
            .contains(&key)
                || values.insert(key, value).is_some()
            {
                return Err("Unknown or duplicate installer preference.".into());
            }
        }
        let boolean = |key| match values.get(key).copied() {
            Some("true") => Ok(true),
            Some("false") => Ok(false),
            _ => Err("Installer switches must be true or false.".to_owned()),
        };
        Ok(Self {
            theme: match values.get("--theme").copied() {
                Some("light") => Theme::Light,
                Some("ink") => Theme::Ink,
                _ => return Err("Unknown installer theme.".into()),
            },
            hotkey: match values.get("--hotkey").copied() {
                Some("ctrl-alt-space") => Hotkey::CtrlAltSpace,
                Some("alt-space") => Hotkey::AltSpace,
                Some("ctrl-shift-space") => Hotkey::CtrlShiftSpace,
                _ => return Err("Unknown installer shortcut.".into()),
            },
            lite: boolean("--lite")?,
            hide_on_focus_loss: boolean("--hide-on-focus-loss")?,
            history: boolean("--history")?,
        })
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        // Invalid existing profiles fail explicitly; never replace them with defaults.
        let mut settings = Settings::load(path)?;
        settings.theme = self.theme;
        settings.hotkey = self.hotkey;
        settings.lite = self.lite;
        settings.hide_on_focus_loss = self.hide_on_focus_loss;
        settings.remember_history = self.history;
        if !self.history {
            settings.history.clear();
        }
        settings.save(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn choices() -> Vec<String> {
        [
            "--theme=ink",
            "--hotkey=alt-space",
            "--lite=true",
            "--hide-on-focus-loss=false",
            "--history=false",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    }
    #[test]
    fn installer_input_is_finite_and_complete() {
        assert!(Choices::parse(&choices()).is_ok());
        for invalid in [
            "--theme=unknown",
            "--theme=ink;shutdown",
            "--path=C:\\other",
            "--theme=ink\0",
            "--history=0",
            "--history=true",
            "--theme",
        ] {
            let mut args = choices();
            args[0] = invalid.into();
            assert!(Choices::parse(&args).is_err(), "{invalid:?}");
        }
        assert!(Choices::parse(&[]).is_err());
    }
    #[test]
    fn setup_preserves_unrelated_preferences_and_rejects_damaged_profiles() {
        let root = std::env::temp_dir().join(format!("throw-install-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("settings.json");
        let mut old = Settings {
            bookmarks: true,
            max_results: 75,
            ..Settings::default()
        };
        old.record_launch("fixture".into());
        old.save(&path).unwrap();
        Choices::parse(&choices()).unwrap().save(&path).unwrap();
        let saved = Settings::load(&path).unwrap();
        assert_eq!(saved.theme, Theme::Ink);
        assert_eq!(saved.hotkey, Hotkey::AltSpace);
        assert!(saved.bookmarks && saved.lite);
        assert_eq!(saved.max_results, 75);
        assert_eq!(saved.web_providers, old.web_providers);
        assert!(!saved.remember_history && saved.history.is_empty());
        std::fs::write(&path, b"damaged preferences").unwrap();
        assert!(Choices::parse(&choices()).unwrap().save(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"damaged preferences");
        std::fs::remove_dir_all(root).unwrap();
    }
}
