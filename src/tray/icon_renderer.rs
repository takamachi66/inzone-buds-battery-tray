use anyhow::Result;
use tray_icon::Icon;

use crate::models::battery_status::BatteryStatus;

const ICON_SIZE: u32 = 64;
const BG: [u8; 4] = [24, 28, 33, 230];
const TRACK: [u8; 4] = [52, 58, 66, 255];
const BORDER: [u8; 4] = [132, 142, 156, 255];
const GREEN: [u8; 4] = [70, 190, 80, 255];
const RED: [u8; 4] = [255, 68, 68, 255];
const UNKNOWN: [u8; 4] = [150, 150, 150, 255];

pub fn make_status_icon(status: &BatteryStatus) -> Result<Icon> {
    let mut rgba = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    fill_background(&mut rgba);

    draw_battery_bar(&mut rgba, 10, status.left);
    draw_battery_bar(&mut rgba, 38, status.right);

    Ok(Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)?)
}

fn fill_background(buffer: &mut [u8]) {
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            paint_pixel(buffer, ICON_SIZE, x, y, BG);
        }
    }
}

fn draw_battery_bar(buffer: &mut [u8], x: u32, level: Option<u8>) {
    let outer_x = x;
    let outer_y = 6;
    let outer_width = 16;
    let outer_height = 52;

    // Outer frame.
    fill_rect(
        buffer,
        ICON_SIZE,
        outer_x,
        outer_y,
        outer_width,
        outer_height,
        BORDER,
    );

    // Inner track.
    let inner_x = outer_x + 2;
    let inner_y = outer_y + 2;
    let inner_width = outer_width - 4;
    let inner_height = outer_height - 4;
    fill_rect(
        buffer,
        ICON_SIZE,
        inner_x,
        inner_y,
        inner_width,
        inner_height,
        TRACK,
    );

    match level {
        Some(value) => {
            let filled_height = if value == 0 {
                0
            } else {
                ((value as u32 * inner_height) / 100).max(1)
            };
            if filled_height > 0 {
                let fill_y = inner_y + (inner_height - filled_height);
                fill_rect(
                    buffer,
                    ICON_SIZE,
                    inner_x,
                    fill_y,
                    inner_width,
                    filled_height,
                    level_color(value),
                );
            }
        }
        None => {
            // Unknown state marker.
            let marker_y = inner_y + (inner_height / 2) - 2;
            fill_rect(
                buffer,
                ICON_SIZE,
                inner_x,
                marker_y,
                inner_width,
                4,
                UNKNOWN,
            );
        }
    }
}

fn level_color(value: u8) -> [u8; 4] {
    if value <= 20 {
        RED
    } else {
        GREEN
    }
}

fn fill_rect(
    buffer: &mut [u8],
    width: u32,
    x: u32,
    y: u32,
    rect_width: u32,
    rect_height: u32,
    color: [u8; 4],
) {
    for py in y..(y + rect_height) {
        for px in x..(x + rect_width) {
            if px < ICON_SIZE && py < ICON_SIZE {
                paint_pixel(buffer, width, px, py, color);
            }
        }
    }
}

fn paint_pixel(buffer: &mut [u8], width: u32, x: u32, y: u32, color: [u8; 4]) {
    let offset = ((y * width + x) * 4) as usize;
    buffer[offset] = color[0];
    buffer[offset + 1] = color[1];
    buffer[offset + 2] = color[2];
    buffer[offset + 3] = color[3];
}
