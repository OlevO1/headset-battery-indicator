use anyhow::{Context, Result};
#[cfg(feature = "portable")]
use std::{fs, path::PathBuf};
#[cfg(not(feature = "portable"))]
use winreg::enums::HKEY_CURRENT_USER;

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub notifications_enabled: bool,
    pub use_number_icon: bool,
    pub sidetone_enabled: bool,
    pub microphone_light_enabled: bool,
    pub inactive_time_minutes: u8,
}

#[cfg(not(feature = "portable"))]
impl Settings {
    pub fn load() -> Result<Self> {
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey("Software\\HeadsetBatteryIndicator")
            .context("accessing registry key")?;

        let notifications_enabled: u32 = key.get_value("NotificationsEnabled").unwrap_or_default();
        let use_number_icon: u32 = key.get_value("UseNumberIcon").unwrap_or_default();
        let sidetone_enabled: u32 = key.get_value("SidetoneEnabled").unwrap_or_default();
        let microphone_light_enabled: u32 =
            key.get_value("MicrophoneLightEnabled").unwrap_or_default();
        let inactive_time_minutes: u32 = key.get_value("InactiveTimeMinutes").unwrap_or_default();

        let settings = Self {
            notifications_enabled: notifications_enabled != 0,
            use_number_icon: use_number_icon != 0,
            sidetone_enabled: sidetone_enabled != 0,
            microphone_light_enabled: microphone_light_enabled != 0,
            inactive_time_minutes: inactive_time_minutes.min(u8::MAX as u32) as u8,
        };
        log::debug!("Loaded settings: {:?}", settings);
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        let hkcu = winreg::RegKey::predef(HKEY_CURRENT_USER);
        let (key, _) = hkcu
            .create_subkey("Software\\HeadsetBatteryIndicator")
            .context("accessing registry key")?;

        key.set_value("NotificationsEnabled", &(self.notifications_enabled as u32))
            .context("setting NotificationsEnabled value")?;

        key.set_value("UseNumberIcon", &(self.use_number_icon as u32))
            .context("setting UseNumberIcon value")?;

        key.set_value("SidetoneEnabled", &(self.sidetone_enabled as u32))
            .context("setting SidetoneEnabled value")?;

        key.set_value(
            "MicrophoneLightEnabled",
            &(self.microphone_light_enabled as u32),
        )
        .context("setting MicrophoneLightEnabled value")?;

        key.set_value("InactiveTimeMinutes", &(self.inactive_time_minutes as u32))
            .context("setting InactiveTimeMinutes value")?;

        Ok(())
    }
}

#[cfg(feature = "portable")]
impl Settings {
    fn get_settings_path() -> Result<PathBuf> {
        let mut path = std::env::current_exe().context("getting current executable path")?;
        path.set_file_name("settings.ini");
        Ok(path)
    }

    pub fn load() -> Result<Self> {
        let path = Self::get_settings_path()?;

        let mut settings = Self {
            notifications_enabled: false,
            use_number_icon: false,
            sidetone_enabled: false,
            microphone_light_enabled: false,
            inactive_time_minutes: 0,
        };

        if path.exists() {
            let contents = fs::read_to_string(&path).context("reading settings.ini")?;
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with("NotificationsEnabled=") {
                    settings.notifications_enabled = line.ends_with("true") || line.ends_with("1");
                } else if line.starts_with("UseNumberIcon=") {
                    settings.use_number_icon = line.ends_with("true") || line.ends_with("1");
                } else if line.starts_with("SidetoneEnabled=") {
                    settings.sidetone_enabled = line.ends_with("true") || line.ends_with("1");
                } else if line.starts_with("MicrophoneLightEnabled=") {
                    settings.microphone_light_enabled =
                        line.ends_with("true") || line.ends_with("1");
                } else if let Some(value) = line.strip_prefix("InactiveTimeMinutes=") {
                    settings.inactive_time_minutes = value.parse().unwrap_or(0);
                }
            }
        } else {
            // Create default settings file if it doesn't exist
            let _ = settings.save();
        }

        log::debug!("Loaded settings from ini: {:?}", settings);
        Ok(settings)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_settings_path()?;

        let contents = format!(
            "NotificationsEnabled={}\nUseNumberIcon={}\nSidetoneEnabled={}\nMicrophoneLightEnabled={}\nInactiveTimeMinutes={}\n",
            self.notifications_enabled,
            self.use_number_icon,
            self.sidetone_enabled,
            self.microphone_light_enabled,
            self.inactive_time_minutes
        );

        fs::write(path, contents).context("writing settings.ini")?;

        Ok(())
    }
}
