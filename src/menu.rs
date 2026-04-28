use std::ffi::OsStr;
use std::fmt::Debug;

use anyhow::Context;
use log::error;
use tray_icon::menu::MenuEvent;
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use winit::event_loop;

use crate::headset_control::ControlCapabilities;
use crate::lang;
use crate::lang::Key::*;
use crate::settings::Settings;

pub enum ControlAction {
    Sidetone,
    MicrophoneLight,
    InactiveTime(u8),
}

const INACTIVE_TIME_OPTIONS: &[(u8, &str)] = &[
    (0, "Off"),
    (5, "5 min"),
    (15, "15 min"),
    (30, "30 min"),
    (60, "60 min"),
];

pub struct ContextMenu {
    pub menu: Menu,
    pub menu_enable_notifications: CheckMenuItem,
    pub menu_show_text_icon: CheckMenuItem,
    menu_logs: MenuItem,
    menu_close: MenuItem,
    menu_update_available: Option<MenuItem>,
    control_menu: Option<ControlMenu>,
    control_capabilities: Option<ControlCapabilities>,
}

struct ControlMenu {
    submenu: Submenu,
    sidetone: CheckMenuItem,
    microphone_light: CheckMenuItem,
    _inactive_time: Submenu,
    inactive_time_items: Vec<(u8, CheckMenuItem)>,
}

impl ContextMenu {
    pub fn new(settings: Settings) -> anyhow::Result<Self> {
        let menu = Menu::new();

        menu.append(&MenuItem::new(
            format!("{} v{}", lang::t(version), crate::VERSION),
            false,
            None,
        ))?;

        let menu_notifications = CheckMenuItem::new(
            lang::t(show_notifications),
            true,
            settings.notifications_enabled,
            None,
        );

        let menu_show_text_icon = CheckMenuItem::new(
            lang::t(show_text_icon),
            true,
            settings.use_number_icon,
            None,
        );

        let menu_logs = MenuItem::new(lang::t(view_logs), true, None);
        let menu_close = MenuItem::new(lang::t(quit_program), true, None);

        menu.append_items(&[&menu_notifications, &menu_show_text_icon, &menu_logs])?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&menu_close)?;

        Ok(Self {
            menu,
            menu_enable_notifications: menu_notifications,
            menu_show_text_icon,
            menu_logs,
            menu_close,
            menu_update_available: None,
            control_menu: None,
            control_capabilities: None,
        })
    }

    /// Shows an "Update available" menu item at the top of the menu
    pub fn show_update_available(&mut self) -> anyhow::Result<()> {
        if self.menu_update_available.is_some() {
            return Ok(()); // Already showing
        }

        let update_text = format!("❗ {}", lang::t(update_available));
        let menu_item = MenuItem::new(update_text, true, None);

        // Insert at position 1 (after version item)
        self.menu.insert(&menu_item, 1)?;
        self.menu_update_available = Some(menu_item);

        Ok(())
    }

    pub fn sync_control_menu(
        &mut self,
        capabilities: ControlCapabilities,
        settings: Settings,
    ) -> anyhow::Result<()> {
        if capabilities.has_controls() {
            if self.control_capabilities != Some(capabilities) {
                self.rebuild_control_menu(capabilities, settings)?;
            }
            self.update_control_checks(settings);
        } else {
            self.remove_control_menu()?;
            self.control_capabilities = None;
        }

        Ok(())
    }

    pub fn clear_control_menu(&mut self) -> anyhow::Result<()> {
        self.remove_control_menu()?;
        self.control_capabilities = None;
        Ok(())
    }

    pub fn control_action_for_event(&self, event: &MenuEvent) -> Option<ControlAction> {
        let control_menu = self.control_menu.as_ref()?;

        if *control_menu.sidetone.id() == event.id {
            return Some(ControlAction::Sidetone);
        }

        if *control_menu.microphone_light.id() == event.id {
            return Some(ControlAction::MicrophoneLight);
        }

        for (minutes, item) in &control_menu.inactive_time_items {
            if *item.id() == event.id {
                return Some(ControlAction::InactiveTime(*minutes));
            }
        }

        None
    }

    pub fn set_sidetone_checked(&self, checked: bool) {
        if let Some(control_menu) = self.control_menu.as_ref() {
            control_menu.sidetone.set_checked(checked);
        }
    }

    pub fn set_microphone_light_checked(&self, checked: bool) {
        if let Some(control_menu) = self.control_menu.as_ref() {
            control_menu.microphone_light.set_checked(checked);
        }
    }

    pub fn set_inactive_time_checked(&self, selected_minutes: u8) {
        if let Some(control_menu) = self.control_menu.as_ref() {
            for (minutes, item) in &control_menu.inactive_time_items {
                item.set_checked(*minutes == selected_minutes);
            }
        }
    }

    fn rebuild_control_menu(
        &mut self,
        capabilities: ControlCapabilities,
        settings: Settings,
    ) -> anyhow::Result<()> {
        self.remove_control_menu()?;

        let submenu = Submenu::new(lang::t(control_headset), true);

        let sidetone_item = CheckMenuItem::new(
            lang::t(sidetone),
            capabilities.sidetone,
            settings.sidetone_enabled,
            None,
        );
        submenu.append(&sidetone_item)?;

        let microphone_light_item = CheckMenuItem::new(
            lang::t(microphone_light),
            capabilities.microphone_light,
            settings.microphone_light_enabled,
            None,
        );
        submenu.append(&microphone_light_item)?;

        let inactive_time_menu = Submenu::new(lang::t(inactive_time), capabilities.inactive_time);
        let mut inactive_time_items = Vec::new();
        for (minutes, label) in INACTIVE_TIME_OPTIONS {
            let item = CheckMenuItem::new(
                *label,
                capabilities.inactive_time,
                *minutes == settings.inactive_time_minutes,
                None,
            );
            inactive_time_menu.append(&item)?;
            inactive_time_items.push((*minutes, item));
        }
        submenu.append(&inactive_time_menu)?;

        let position = self
            .menu
            .items()
            .iter()
            .position(|item| *item.id() == *self.menu_logs.id())
            .unwrap_or_else(|| self.menu.items().len());

        self.menu.insert(&submenu, position)?;
        self.control_menu = Some(ControlMenu {
            submenu,
            sidetone: sidetone_item,
            microphone_light: microphone_light_item,
            _inactive_time: inactive_time_menu,
            inactive_time_items,
        });
        self.control_capabilities = Some(capabilities);

        Ok(())
    }

    fn remove_control_menu(&mut self) -> anyhow::Result<()> {
        if let Some(control_menu) = self.control_menu.take() {
            self.menu.remove(&control_menu.submenu)?;
        }

        Ok(())
    }

    fn update_control_checks(&self, settings: Settings) {
        self.set_sidetone_checked(settings.sidetone_enabled);
        self.set_microphone_light_checked(settings.microphone_light_enabled);
        self.set_inactive_time_checked(settings.inactive_time_minutes);
    }

    pub fn handle_event(&mut self, event: MenuEvent, event_loop: &event_loop::ActiveEventLoop) {
        match event.id {
            id if id == self.menu_close.id() => event_loop.exit(),

            id if self
                .menu_update_available
                .as_ref()
                .is_some_and(|m| *m.id() == id) =>
            {
                let url = "https://github.com/aarol/headset-battery-indicator/releases";
                {
                    if let Err(err) = Self::spawn_command_no_window("explorer", &[&url]) {
                        error!("Failed to open update URL {url}: {err:?}");
                    }
                }
            }
            id if id == self.menu_logs.id() => {
                if let Ok(dir) = std::env::current_exe().map(|p| p.parent().unwrap().to_path_buf())
                {
                    let path = dir.join("headset-battery-indicator.log");
                    {
                        if let Err(e) = Self::spawn_command_no_window("explorer", &[&path]) {
                            error!("Failed to open log file at {}: {e:?}", path.display());
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn spawn_command_no_window<S>(cmd: &str, args: &[S]) -> anyhow::Result<()>
    where
        S: AsRef<OsStr> + Debug,
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;

        let mut command = std::process::Command::new(cmd);
        command.args(args).creation_flags(CREATE_NO_WINDOW);

        command
            .spawn()
            .with_context(|| format!("Failed to spawn command: {} {:?}", cmd, args))?;

        Ok(())
    }
}
