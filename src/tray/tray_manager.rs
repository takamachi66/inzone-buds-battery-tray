use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{TrayIcon, TrayIconBuilder};

use crate::models::battery_status::BatteryStatus;
use crate::tray::icon_renderer::make_status_icon;

pub struct TrayManager {
    _menu: Menu,
    _tray_icon: TrayIcon,
    summary_item: MenuItem,
    status_item: MenuItem,
    left_item: MenuItem,
    right_item: MenuItem,
    case_item: MenuItem,
    exit_item: MenuItem,
}

impl TrayManager {
    pub fn new() -> Result<Self> {
        let menu = Menu::new();
        let summary_item =
            MenuItem::with_id(MenuId::new("summary"), "INZONE Buds: --", false, None);
        let status_item = MenuItem::with_id(MenuId::new("status"), "Status: Starting", false, None);
        let left_item = MenuItem::with_id(MenuId::new("left"), "Left: --", false, None);
        let right_item = MenuItem::with_id(MenuId::new("right"), "Right: --", false, None);
        let case_item = MenuItem::with_id(MenuId::new("case"), "Case: --", false, None);
        let exit_item = MenuItem::with_id(MenuId::new("exit"), "Exit", true, None);

        menu.append(&summary_item)?;
        menu.append(&left_item)?;
        menu.append(&right_item)?;
        menu.append(&case_item)?;
        menu.append(&status_item)?;
        menu.append(&exit_item)?;

        let tray_icon = TrayIconBuilder::new()
            .with_tooltip("INZONE Buds")
            .with_icon(make_status_icon(&BatteryStatus::disconnected())?)
            .with_menu(Box::new(menu.clone()))
            .build()?;

        Ok(Self {
            _menu: menu,
            _tray_icon: tray_icon,
            summary_item,
            status_item,
            left_item,
            right_item,
            case_item,
            exit_item,
        })
    }

    pub fn update_status(&mut self, status: &BatteryStatus) -> Result<()> {
        let [status_line, left_line, right_line, case_line] = status.summary_lines();
        let summary = format_summary(status);
        let has_displayable_values = status.has_displayable_values();
        self.summary_item.set_text(&summary);
        self.status_item.set_text(&status_line);
        self.left_item.set_text(&left_line);
        self.right_item.set_text(&right_line);
        self.case_item.set_text(&case_line);

        let tooltip = if has_displayable_values {
            summary
        } else {
            "INZONE Buds: Disconnected".to_string()
        };

        self._tray_icon.set_tooltip(Some(tooltip))?;
        self._tray_icon.set_icon(Some(make_status_icon(status)?))?;

        Ok(())
    }

    pub fn exit_item_id(&self) -> MenuId {
        self.exit_item.id().clone()
    }
}

fn format_summary(status: &BatteryStatus) -> String {
    if !status.has_displayable_values() {
        return "INZONE Buds: Disconnected".to_string();
    }

    format!(
        "INZONE Buds: L {} / R {} / C {}",
        BatteryStatus::format_level(status.left),
        BatteryStatus::format_level(status.right),
        BatteryStatus::format_level(status.case)
    )
}
