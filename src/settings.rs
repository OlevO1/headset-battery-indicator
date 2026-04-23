use anyhow::{Context, Result};
#[cfg(feature = "portable")]
use std::{fs, path::PathBuf};
#[cfg(not(feature = "portable"))]
use winreg::enums::HKEY_CURRENT_USER;

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub notifications_enabled: bool,
    pub use_number_icon: bool,
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

        let settings = Self {
            notifications_enabled: notifications_enabled != 0,
            use_number_icon: use_number_icon != 0,
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
        };

        if path.exists() {
            let contents = fs::read_to_string(&path).context("reading settings.ini")?;
            for line in contents.lines() {
                let line = line.trim();
                if line.starts_with("NotificationsEnabled=") {
                    settings.notifications_enabled = line.ends_with("true") || line.ends_with("1");
                } else if line.starts_with("UseNumberIcon=") {
                    settings.use_number_icon = line.ends_with("true") || line.ends_with("1");
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
            "NotificationsEnabled={}\nUseNumberIcon={}\n",
            self.notifications_enabled, self.use_number_icon
        );

        fs::write(path, contents).context("writing settings.ini")?;

        Ok(())
    }
}
