use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

pub struct TrayManager {
    pub tray_icon: TrayIcon,
    pub toggle_item: MenuItem,
    pub show_item: MenuItem,
    pub exit_item: MenuItem,
}

impl TrayManager {
    pub fn new() -> anyhow::Result<Self> {
        let menu = Menu::new();

        let toggle_item = MenuItem::new("Start All Watchers", true, None);
        let show_item = MenuItem::new("Show Dashboard", true, None);
        let exit_item = MenuItem::new("Exit Roblox Automator", true, None);

        let _ = menu.append(&show_item);
        let _ = menu.append(&toggle_item);
        let _ = menu.append(&PredefinedMenuItem::separator());
        let _ = menu.append(&exit_item);

        let icon = Self::create_icon(false);

        let tray_icon = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Roblox Automator (Idle)")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            tray_icon,
            toggle_item,
            show_item,
            exit_item,
        })
    }

    pub fn set_running_state(&self, running: bool, win_count: usize) {
        let icon = Self::create_icon(running);
        let _ = self.tray_icon.set_icon(Some(icon));

        let tooltip = if running {
            format!("Roblox Automator: Monitoring ({} windows)", win_count)
        } else {
            format!("Roblox Automator: Idle ({} windows)", win_count)
        };
        let _ = self.tray_icon.set_tooltip(Some(tooltip));

        let text = if running { "Stop All Watchers" } else { "Start All Watchers" };
        self.toggle_item.set_text(text);
    }

    /// Creates an in-memory 16x16 RGBA icon for the system tray
    pub fn create_icon(active: bool) -> Icon {
        let size = 16;
        let mut rgba = Vec::with_capacity(size * size * 4);

        for y in 0..size {
            for x in 0..size {
                let dist = ((x as f32 - 7.5).powi(2) + (y as f32 - 7.5).powi(2)).sqrt();
                if dist <= 6.5 {
                    if active {
                        // Cyan #38BDF8
                        rgba.extend_from_slice(&[56, 189, 248, 255]);
                    } else {
                        // Slate #64748B
                        rgba.extend_from_slice(&[100, 116, 139, 255]);
                    }
                } else {
                    // Transparent
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }

        Icon::from_rgba(rgba, size as u32, size as u32).expect("valid tray icon")
    }
}
