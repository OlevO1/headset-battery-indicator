use anyhow::{Context, Result};
use winreg::enums::HKEY_CURRENT_USER;

#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub notifications_enabled: bool,
    pub use_number_icon: bool,
}

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
