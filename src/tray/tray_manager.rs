use anyhow::Result;
use tray_icon::menu::{Menu, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::models::battery_status::BatteryStatus;

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
            .with_icon(make_icon(0, false)?)
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
        self.summary_item.set_text(&summary);
        self.status_item.set_text(&status_line);
        self.left_item.set_text(&left_line);
        self.right_item.set_text(&right_line);
        self.case_item.set_text(&case_line);

        let tooltip = if status.connected {
            summary
        } else {
            "INZONE Buds: Disconnected".to_string()
        };

        self._tray_icon.set_tooltip(Some(tooltip))?;
        self._tray_icon.set_icon(Some(make_icon(
            status.min_percent().unwrap_or(0),
            status.connected,
        )?))?;

        Ok(())
    }

    pub fn exit_item_id(&self) -> MenuId {
        self.exit_item.id().clone()
    }
}

fn format_summary(status: &BatteryStatus) -> String {
    if !status.connected {
        return "INZONE Buds: Disconnected".to_string();
    }

    format!(
        "INZONE Buds: L {} / R {} / C {}",
        BatteryStatus::format_level(status.left),
        BatteryStatus::format_level(status.right),
        BatteryStatus::format_level(status.case)
    )
}

fn make_icon(percent: u8, connected: bool) -> Result<Icon> {
    const SIZE: u32 = 32;
    let mut rgba = vec![0_u8; (SIZE * SIZE * 4) as usize];

    let (r, g, b) = if !connected {
        (100_u8, 100_u8, 100_u8)
    } else if percent <= 20 {
        (210, 60, 55)
    } else if percent <= 50 {
        (224, 165, 45)
    } else {
        (70, 170, 80)
    };

    for y in 6..26 {
        for x in 5..23 {
            paint_pixel(&mut rgba, SIZE, x, y, r, g, b, 255);
        }
    }

    for y in 12..20 {
        for x in 23..27 {
            paint_pixel(&mut rgba, SIZE, x, y, r, g, b, 255);
        }
    }

    for y in 8..24 {
        for x in 7..21 {
            paint_pixel(&mut rgba, SIZE, x, y, 30, 35, 40, 255);
        }
    }

    if connected {
        let width = ((percent as u32 * 12) / 100).max(1);
        for y in 10..22 {
            for x in 8..(8 + width) {
                paint_pixel(&mut rgba, SIZE, x, y, r, g, b, 255);
            }
        }
    }

    Ok(Icon::from_rgba(rgba, SIZE, SIZE)?)
}

fn paint_pixel(buffer: &mut [u8], width: u32, x: u32, y: u32, r: u8, g: u8, b: u8, a: u8) {
    let offset = ((y * width + x) * 4) as usize;
    buffer[offset] = r;
    buffer[offset + 1] = g;
    buffer[offset + 2] = b;
    buffer[offset + 3] = a;
}
