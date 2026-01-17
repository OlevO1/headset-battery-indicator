use std::ffi::OsStr;
use std::fmt::Debug;

use anyhow::Context;
use log::error;
use tray_icon::menu::MenuEvent;
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use winit::event_loop;

use crate::headset_control;
use crate::lang;
use crate::lang::Key::*;

pub struct ContextMenu {
    pub menu: Menu,
    device_menu_items: Vec<(headset_control::Device, CheckMenuItem)>,
    pub selected_device_idx: usize,
    separators: Option<(PredefinedMenuItem, PredefinedMenuItem)>, // (top, bottom)
    pub menu_notifications: CheckMenuItem,
    menu_logs: MenuItem,
    menu_close: MenuItem,
    pub menu_trigger_notification: MenuItem,
    menu_update_available: Option<MenuItem>,
}

impl ContextMenu {
    pub fn new(notifications_enabled: bool) -> anyhow::Result<Self> {
        let menu = Menu::new();

        menu.append(&MenuItem::new(
            format!("{} v{}", lang::t(version), crate::VERSION),
            false,
            None,
        ))?;

        let device_menu_items = Vec::new();

        let menu_notifications = CheckMenuItem::new(
            lang::t(show_notifications),
            true,
            notifications_enabled,
            None,
        );

        let menu_logs = MenuItem::new(lang::t(view_logs), true, None);
        let menu_close = MenuItem::new(lang::t(quit_program), true, None);
        let separators = None;
        let menu_trigger_notification = MenuItem::new("Trigger Test Notification", true, None);

        #[cfg(debug_assertions)]
        menu.append(&menu_trigger_notification)?;

        menu.append_items(&[&menu_notifications, &menu_logs])?;
        menu.append(&PredefinedMenuItem::separator())?;
        menu.append(&menu_close)?;

        Ok(Self {
            menu,
            device_menu_items,
            selected_device_idx: 0,
            separators,
            menu_notifications,
            menu_logs,
            menu_close,
            menu_trigger_notification,
            menu_update_available: None,
        })
    }

    /// Shows an "Update available" menu item at the top of the menu
    pub fn show_update_available(&mut self) -> anyhow::Result<()> {
        if self.menu_update_available.is_some() {
            return Ok(()); // Already showing
        }

        let update_text = format!("🛎️ {}", lang::t(update_available));
        let menu_item = MenuItem::new(update_text, true, None);

        // Insert at position 1 (after version item)
        self.menu.insert(&menu_item, 1)?;
        self.menu_update_available = Some(menu_item);

        Ok(())
    }

    pub fn update_device_menu(
        &mut self,
        devices: &[headset_control::Device],
    ) -> anyhow::Result<()> {
        // Remove separators
        if let Some((top, bottom)) = &self.separators {
            self.menu.remove(top).context("Removing top separator")?;
            self.menu
                .remove(bottom)
                .context("Removing bottom separator")?;
            self.separators = None;
        }

        // Remove old device menu items
        for (_, item) in &self.device_menu_items {
            self.menu.remove(item)?;
        }
        if devices.is_empty() {
            self.selected_device_idx = 0;
            return Ok(());
        }

        let (top_separator, bottom_separator) = (
            PredefinedMenuItem::separator(),
            PredefinedMenuItem::separator(),
        );

        self.device_menu_items.clear();
        self.menu.insert(&top_separator, 1)?;

        self.selected_device_idx = self.selected_device_idx.min(devices.len() - 1);

        // Add new device menu items
        for (i, device) in devices.iter().enumerate() {
            let is_selected = i == self.selected_device_idx;
            let menu_item = CheckMenuItem::new(device.product.clone(), true, is_selected, None);
            self.menu.insert(&menu_item, 2 + i)?; // Insert after version item
            self.device_menu_items.push((device.clone(), menu_item));
        }

        self.menu.insert(&bottom_separator, 2 + devices.len())?;
        self.separators = Some((top_separator, bottom_separator));

        Ok(())
    }

    fn set_selected(&mut self, idx: usize) {
        if idx >= self.device_menu_items.len() {
            return;
        }

        for (i, (_, item)) in self.device_menu_items.iter().enumerate() {
            item.set_checked(i == idx);
        }
        self.selected_device_idx = idx;
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
            id => {
                let idx = self
                    .device_menu_items
                    .iter()
                    .enumerate()
                    .find(|(_, (_, m))| m.id() == &id);
                if let Some((i, _)) = idx {
                    self.set_selected(i);
                }
            }
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
