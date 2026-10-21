use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read, Write},
    path::Path,
};

pub const MAX_SETTINGS_BYTES: u64 = 64 * 1024;
pub const MAX_HISTORY: usize = 100;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub version: u8,
    pub lite: bool,
    pub hide_on_startup: bool,
    pub hide_on_focus_loss: bool,
    pub calculator: bool,
    pub web_search: bool,
    pub bookmarks: bool,
    pub web_providers: Vec<crate::web::Provider>,
    pub remember_history: bool,
    pub hotkey: Hotkey,
    pub theme: Theme,
    pub max_results: usize,
    pub history: BTreeMap<String, u32>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum Hotkey {
    #[default]
    CtrlAltSpace,
    AltSpace,
    CtrlShiftSpace,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
pub enum Theme {
    Ink,
    #[default]
    Light,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: 1,
            lite: true,
            hide_on_startup: false,
            hide_on_focus_loss: true,
            calculator: true,
            web_search: true,
            bookmarks: false,
            web_providers: crate::web::defaults(),
            remember_history: true,
            hotkey: Hotkey::default(),
            theme: Theme::default(),
            max_results: 50,
            history: BTreeMap::new(),
        }
    }
}

impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        crate::web::validate_providers(&self.web_providers)?;
        if self.version != 1 {
            return Err("Unsupported settings version.".into());
        }
        if !(4..=100).contains(&self.max_results) {
            return Err("Search result limit must be between 4 and 100.".into());
        }
        if self.history.len() > MAX_HISTORY
            || self
                .history
                .keys()
                .any(|k| k.len() > 4096 || k.contains('\0'))
        {
            return Err("Launch history exceeds safe limits.".into());
        }
        Ok(())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let bytes = read_bounded(path, MAX_SETTINGS_BYTES).map_err(|e| e.to_string())?;
        let mut settings: Self =
            serde_json::from_slice(&bytes).map_err(|e| format!("Invalid settings: {e}"))?;
        settings.validate()?;
        // Older releases used this as the visible-row count. It now caps the
        // scrollable search list; the viewport is always three rows.
        if settings.max_results <= 12 {
            settings.max_results = 50;
        }
        Ok(settings)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() as u64 > MAX_SETTINGS_BYTES {
            return Err("Settings file is too large.".into());
        }
        atomic_write(path, &bytes).map_err(|e| format!("Could not save settings: {e}"))
    }

    pub fn record_launch(&mut self, key: String) {
        if !self.remember_history || key.len() > 4096 || key.contains('\0') {
            return;
        }
        if self.history.len() >= MAX_HISTORY && !self.history.contains_key(&key) {
            if let Some(least) = self
                .history
                .iter()
                .min_by_key(|(_, count)| **count)
                .map(|(key, _)| key.clone())
            {
                self.history.remove(&least);
            }
        }
        let count = self.history.entry(key).or_default();
        *count = count.saturating_add(1);
    }
}

pub fn read_bounded(path: &Path, max: u64) -> io::Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File exceeds size limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(max + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "File exceeds size limit",
        ));
    }
    Ok(bytes)
}

/// Write then replace, preserving the old file if serialization or writing fails.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Missing parent directory"))?;
    fs::create_dir_all(parent)?;
    // Never truncate/follow a predictable temporary file left by another process.
    static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    struct Cleanup<'a>(&'a Path);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.0);
        }
    }
    let _cleanup = Cleanup(&temp);
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let src: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let dst: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        if unsafe {
            MoveFileExW(
                src.as_ptr(),
                dst.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(not(windows))]
    fs::rename(&temp, path)?;
    Ok(())
}

/// Copy only supported preferences; never copy paths, executable commands or plugin code.
pub fn import_preferences(bytes: &[u8], settings: &mut Settings) -> Result<(), String> {
    if bytes.len() > 1024 * 1024 {
        return Err("Import exceeds 1 MB.".into());
    }
    // Accept Flow's additional fields, but reject ambiguous duplicate root keys.
    struct UniqueObject(BTreeMap<String, serde_json::Value>);
    impl<'de> Deserialize<'de> for UniqueObject {
        fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
            struct Visitor;
            impl<'de> serde::de::Visitor<'de> for Visitor {
                type Value = UniqueObject;
                fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                    formatter.write_str("a settings object without duplicate keys")
                }
                fn visit_map<M: serde::de::MapAccess<'de>>(
                    self,
                    mut map: M,
                ) -> Result<Self::Value, M::Error> {
                    let mut object = BTreeMap::new();
                    while let Some((key, value)) = map.next_entry::<String, serde_json::Value>()? {
                        if object.insert(key, value).is_some() {
                            return Err(serde::de::Error::custom("Duplicate import key"));
                        }
                    }
                    Ok(UniqueObject(object))
                }
            }
            deserializer.deserialize_map(Visitor)
        }
    }
    let UniqueObject(object) = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    let mut next = settings.clone();
    let mut count = 0;
    for (name, target) in [
        ("HideOnStartup", &mut next.hide_on_startup),
        ("HideWhenDeactivated", &mut next.hide_on_focus_loss),
    ] {
        if let Some(value) = object.get(name) {
            *target = value
                .as_bool()
                .ok_or("Imported preferences must be booleans.")?;
            count += 1;
        }
    }
    if count == 0 {
        return Err("No supported preferences found.".into());
    }
    next.lite = true;
    *settings = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lite_defaults_and_validation() {
        let mut s = Settings::default();
        assert!(s.lite);
        assert_eq!(s.hotkey, Hotkey::CtrlAltSpace);
        s.max_results = 500;
        assert!(s.validate().is_err());
        assert!(serde_json::from_str::<Settings>(r#"{"hotkey":"cmd.exe"}"#).is_err());
        assert!(serde_json::from_str::<Settings>(r#"{"version":1,"version":2}"#).is_err());
        assert!(serde_json::from_str::<Settings>(r#"{"command":"evil"}"#).is_err());
    }
    #[test]
    fn history_is_bounded_and_optional() {
        let mut s = Settings::default();
        for i in 0..300 {
            s.record_launch(format!("app-{i}"));
        }
        assert_eq!(s.history.len(), MAX_HISTORY);
        s.remember_history = false;
        s.record_launch("not recorded".into());
        assert!(!s.history.contains_key("not recorded"));
    }
    #[test]
    fn import_cannot_enable_commands_or_disable_lite() {
        let mut s = Settings::default();
        import_preferences(
            br#"{"HideOnStartup":true,"Plugins":[{"command":"bad"}],"LiteMode":false}"#,
            &mut s,
        )
        .unwrap();
        assert!(s.hide_on_startup && s.lite);
        let before = s.clone();
        assert!(import_preferences(
            br#"{"HideOnStartup":false,"HideWhenDeactivated":"bad"}"#,
            &mut s
        )
        .is_err());
        assert_eq!(s, before);
        assert!(import_preferences(b"[]", &mut s).is_err());
        assert!(import_preferences(b"{}", &mut s).is_err());
        assert!(
            import_preferences(br#"{"HideOnStartup":true,"HideOnStartup":false}"#, &mut s).is_err()
        );
    }
    #[test]
    fn settings_roundtrip_and_bounded_read() {
        let root = std::env::temp_dir().join(format!("throw-config-test-{}", std::process::id()));
        let path = root.join("settings.json");
        let mut s = Settings::default();
        s.record_launch("app.lnk".into());
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        s.theme = Theme::Light;
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        assert!(read_bounded(&path, 1).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn web_preferences_migrate_and_reject_invalid_updates_without_data_loss() {
        let old: Settings = serde_json::from_str(r#"{"version":1,"web_search":false}"#).unwrap();
        assert!(!old.bookmarks && !old.web_search && old.lite);
        assert_eq!(old.web_providers, crate::web::defaults());
        assert!(serde_json::from_str::<Settings>(r#"{"web_providers":[{"name":"X","keyword":"x","template":"https://a/{query}","enabled":true,"command":"evil"}]}"#).is_err());
        let root =
            std::env::temp_dir().join(format!("throw-web-config-test-{}", std::process::id()));
        let path = root.join("settings.json");
        let mut s = Settings {
            bookmarks: true,
            ..Settings::default()
        };
        s.web_providers.push(crate::web::Provider {
            name: "Wiki".into(),
            keyword: "wiki".into(),
            template: "https://en.wikipedia.org/w/index.php?search={query}".into(),
            enabled: false,
        });
        s.save(&path).unwrap();
        assert_eq!(Settings::load(&path).unwrap(), s);
        let before = fs::read(&path).unwrap();
        s.web_providers[1].keyword = "?".into();
        assert!(s.save(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        s.web_providers.clear();
        s.save(&path).unwrap();
        assert!(Settings::load(&path).unwrap().web_providers.is_empty());
        fs::write(&path, br#"{"web_providers":[{"name":"Evil","keyword":"x","template":"file:///{query}","enabled":true}]}"#).unwrap();
        assert!(Settings::load(&path).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
