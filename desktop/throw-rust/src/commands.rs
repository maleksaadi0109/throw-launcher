//! Closed command catalog: search text is never interpreted as a shell command.
use crate::search::{Action, ResultRow};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SystemCommand {
    Restart,
    Shutdown,
    AdvancedRestart,
    SignOut,
    Lock,
    Sleep,
    Hibernate,
    EmptyRecycleBin,
}

impl SystemCommand {
    pub fn confirmation(self) -> Option<&'static str> {
        match self {
            Self::Restart => Some("Restart this computer now? Save your work before continuing."),
            Self::Shutdown => {
                Some("Shut down this computer now? Save your work before continuing.")
            }
            Self::AdvancedRestart => {
                Some("Restart into Windows recovery options now? Save your work first.")
            }
            Self::SignOut => Some("Sign out of Windows now? Save your work before continuing."),
            Self::Sleep => Some("Put this computer to sleep now? Active tasks may be interrupted."),
            Self::Hibernate => {
                Some("Hibernate this computer now? Windows must have hibernation enabled.")
            }
            Self::EmptyRecycleBin => {
                Some("Permanently empty the Recycle Bin on all drives? This cannot be undone.")
            }
            Self::Lock => None,
        }
    }

    pub fn authorize(self, confirmed: bool) -> Result<(), &'static str> {
        if self.confirmation().is_some() && !confirmed {
            Err("This action requires explicit confirmation.")
        } else {
            Ok(())
        }
    }

    /// Only constant arguments can reach the trusted Windows shutdown utility.
    /// Never force-close applications, even after confirmation.
    pub fn shutdown_args(self) -> Option<&'static [&'static str]> {
        match self {
            Self::Restart => Some(&["/r", "/t", "0"]),
            Self::Shutdown => Some(&["/s", "/t", "0"]),
            Self::AdvancedRestart => Some(&["/r", "/o", "/t", "0"]),
            Self::SignOut => Some(&["/l"]),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsPage {
    Home,
    Display,
    Sound,
    Bluetooth,
    Network,
    Wifi,
    Apps,
    Update,
    Privacy,
    Power,
    Storage,
    Personalization,
    Accessibility,
}
impl SettingsPage {
    pub fn uri(self) -> &'static str {
        match self {
            Self::Home => "ms-settings:",
            Self::Display => "ms-settings:display",
            Self::Sound => "ms-settings:sound",
            Self::Bluetooth => "ms-settings:bluetooth",
            Self::Network => "ms-settings:network-status",
            Self::Wifi => "ms-settings:network-wifi",
            Self::Apps => "ms-settings:appsfeatures",
            Self::Update => "ms-settings:windowsupdate",
            Self::Privacy => "ms-settings:privacy",
            Self::Power => "ms-settings:powersleep",
            Self::Storage => "ms-settings:storagesense",
            Self::Personalization => "ms-settings:personalization",
            Self::Accessibility => "ms-settings:easeofaccess",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UtilityCommand {
    WindowsSettings(SettingsPage),
    ControlPanel,
    TaskManager,
    RecycleBin,
    IndexOptions,
    CommandPrompt,
    PowerShell,
    ProfileFolder,
    MeasureMemory,
    LightTheme,
    DarkTheme,
}

pub fn search(filter: &str) -> Vec<ResultRow> {
    use SettingsPage::*;
    use SystemCommand::*;
    use UtilityCommand::*;
    let filter = filter.trim().to_lowercase();
    if filter.len() > 512 || filter.chars().any(char::is_control) {
        return vec![];
    }
    let catalog = [
        (
            "Settings",
            "Throw appearance, shortcuts and privacy",
            "preferences throw settings",
            Action::Settings,
        ),
        (
            "Restart computer",
            "System · Save your work; confirmation required",
            "restart reboot إعادة التشغيل",
            Action::System(Restart),
        ),
        (
            "Shut down",
            "System · Save your work; confirmation required",
            "shutdown power off إيقاف التشغيل",
            Action::System(Shutdown),
        ),
        (
            "Lock computer",
            "System · Lock your Windows session",
            "lock workstation قفل",
            Action::System(Lock),
        ),
        (
            "Sleep",
            "System · Suspend your computer; confirmation required",
            "sleep suspend سكون",
            Action::System(Sleep),
        ),
        (
            "Hibernate",
            "System · Requires Windows hibernation support",
            "hibernate إسبات",
            Action::System(Hibernate),
        ),
        (
            "Sign out",
            "System · Save your work; confirmation required",
            "log off logoff logout sign out خروج",
            Action::System(SignOut),
        ),
        (
            "Advanced restart",
            "System · Restart into Windows recovery options",
            "advanced boot recovery safe mode",
            Action::System(AdvancedRestart),
        ),
        (
            "Open Recycle Bin",
            "Windows · Browse deleted items",
            "recycle bin trash سلة المحذوفات",
            Action::Utility(RecycleBin),
        ),
        (
            "Empty Recycle Bin",
            "System · Permanent deletion; confirmation required",
            "empty recycle bin trash",
            Action::System(EmptyRecycleBin),
        ),
        (
            "Windows Settings",
            "Windows · Open system settings",
            "windows settings إعدادات ويندوز",
            Action::Utility(WindowsSettings(Home)),
        ),
        (
            "Display settings",
            "Windows · Screens, resolution and scaling",
            "display screen monitor",
            Action::Utility(WindowsSettings(Display)),
        ),
        (
            "Sound settings",
            "Windows · Audio devices and volume",
            "sound audio volume",
            Action::Utility(WindowsSettings(Sound)),
        ),
        (
            "Bluetooth settings",
            "Windows · Manage Bluetooth devices",
            "bluetooth devices",
            Action::Utility(WindowsSettings(Bluetooth)),
        ),
        (
            "Network settings",
            "Windows · Network status and connections",
            "network internet ethernet",
            Action::Utility(WindowsSettings(Network)),
        ),
        (
            "Wi-Fi settings",
            "Windows · Wireless connections, when available",
            "wifi wi-fi wireless",
            Action::Utility(WindowsSettings(Wifi)),
        ),
        (
            "Installed apps",
            "Windows · Manage installed applications",
            "uninstall installed apps programs",
            Action::Utility(WindowsSettings(Apps)),
        ),
        (
            "Windows Update",
            "Windows · Updates and restart options",
            "windows update updates",
            Action::Utility(WindowsSettings(Update)),
        ),
        (
            "Privacy settings",
            "Windows · App permissions and privacy",
            "privacy permissions",
            Action::Utility(WindowsSettings(Privacy)),
        ),
        (
            "Power settings",
            "Windows · Power and sleep preferences",
            "power battery energy",
            Action::Utility(WindowsSettings(Power)),
        ),
        (
            "Storage settings",
            "Windows · Disk space and storage",
            "storage disk space",
            Action::Utility(WindowsSettings(Storage)),
        ),
        (
            "Personalization",
            "Windows · Background, colors and themes",
            "personalization wallpaper background",
            Action::Utility(WindowsSettings(Personalization)),
        ),
        (
            "Accessibility settings",
            "Windows · Accessibility options",
            "accessibility ease of access",
            Action::Utility(WindowsSettings(Accessibility)),
        ),
        (
            "Control Panel",
            "Windows · Classic system settings",
            "control panel",
            Action::Utility(ControlPanel),
        ),
        (
            "Task Manager",
            "Windows · Processes and performance",
            "task manager processes",
            Action::Utility(TaskManager),
        ),
        (
            "Indexing options",
            "Windows · Configure the Windows Search index",
            "indexing options windows search",
            Action::Utility(IndexOptions),
        ),
        (
            "Command Prompt",
            "Windows · Open a terminal; does not execute your search",
            "cmd command prompt terminal",
            Action::Utility(CommandPrompt),
        ),
        (
            "PowerShell",
            "Windows · Open a terminal without loading a profile",
            "powershell terminal",
            Action::Utility(PowerShell),
        ),
        (
            "Open Throw data folder",
            "Throw · Settings and local search caches",
            "profile user data folder",
            Action::Utility(ProfileFolder),
        ),
        (
            "Measure memory",
            "Throw · Show this process's current memory use",
            "measure memory ram usage",
            Action::Utility(MeasureMemory),
        ),
        (
            "Pearl theme",
            "Throw · Use the light appearance",
            "pearl light theme",
            Action::Utility(LightTheme),
        ),
        (
            "Graphite theme",
            "Throw · Use the dark appearance",
            "graphite dark theme",
            Action::Utility(DarkTheme),
        ),
        (
            "Rebuild search index",
            "Throw · Refresh applications and local folders",
            "reindex rebuild search index refresh",
            Action::Reindex,
        ),
        (
            "Quit Throw",
            "Throw · Exit the launcher",
            "quit exit throw",
            Action::Quit,
        ),
    ];
    catalog
        .into_iter()
        .filter(|(title, _, aliases, _)| {
            let text = format!("{} {aliases}", title.to_lowercase());
            filter.split_whitespace().all(|term| text.contains(term))
        })
        .map(|(title, detail, _, action)| ResultRow {
            title: title.into(),
            detail: detail.into(),
            action,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_and_arabic_find_typed_commands_only() {
        for input in ["reboot", "restart", "إعادة التشغيل"] {
            assert_eq!(
                search(input)[0].action,
                Action::System(SystemCommand::Restart)
            );
        }
        assert!(search("restart & calc.exe").is_empty());
        assert!(search("cmd /c whoami").is_empty());
        assert!(search("shutdown\0").is_empty());
        assert!(search("ms-settings:evil").is_empty());
        assert!(search(&"a".repeat(513)).is_empty());
        assert!(search("").len() > 30);
    }
    #[test]
    fn disruptive_actions_are_guarded_and_never_force_close() {
        for row in search("") {
            if let Action::System(command) = row.action {
                assert!(command.authorize(true).is_ok());
                if command != SystemCommand::Lock {
                    assert!(command.authorize(false).is_err());
                }
                if let Some(args) = command.shutdown_args() {
                    assert!(!args.contains(&"/f"));
                    assert!(!args.iter().any(|a| a.contains(['&', '|', ';'])));
                }
            }
        }
    }
}
