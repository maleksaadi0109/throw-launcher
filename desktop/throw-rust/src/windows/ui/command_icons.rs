//! Flow's MIT-licensed system-command icons, embedded in the executable.
//! LR_SHARED lets Windows cache these fixed resources; no disk decoding or
//! unbounded per-result cache is needed. Do not DestroyIcon shared handles.
use super::*;
use throw_launcher::commands::{SettingsPage, SystemCommand, UtilityCommand};

pub(super) fn resource_id(action: &Action) -> Option<u16> {
    Some(match action {
        Action::System(command) => match command {
            SystemCommand::Restart => 100,
            SystemCommand::Shutdown => 101,
            SystemCommand::AdvancedRestart => 102,
            SystemCommand::SignOut => 103,
            SystemCommand::Lock => 104,
            SystemCommand::Sleep => 105,
            SystemCommand::Hibernate => 106,
            SystemCommand::EmptyRecycleBin => 107,
        },
        Action::Utility(command) => match command {
            UtilityCommand::RecycleBin => 108,
            UtilityCommand::IndexOptions => 109,
            UtilityCommand::WindowsSettings(SettingsPage::Update) => 110,
            UtilityCommand::LightTheme | UtilityCommand::DarkTheme => 111,
            UtilityCommand::WindowsSettings(_)
            | UtilityCommand::ControlPanel
            | UtilityCommand::TaskManager => 112,
            _ => return None,
        },
        Action::Settings => 111,
        Action::Quit => 113,
        Action::Reindex => 109,
        _ => return None,
    })
}

pub(super) unsafe fn load(instance: HINSTANCE, action: &Action, size: i32) -> HICON {
    resource_id(action).map_or(null_mut(), |id| {
        LoadImageW(
            instance,
            id as usize as *const u16,
            IMAGE_ICON,
            size,
            size,
            LR_SHARED,
        ) as HICON
    })
}
