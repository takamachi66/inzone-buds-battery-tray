use anyhow::Result;
use tray_icon::Icon;

use crate::models::battery_status::BatteryStatus;

const ICON_SIZE: u32 = 64;
const BG: [u8; 4] = [24, 28, 33, 230];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const YELLOW: [u8; 4] = [255, 204, 0, 255];
const RED: [u8; 4] = [255, 68, 68, 255];
const GRAY: [u8; 4] = [160, 160, 160, 255];

pub fn make_status_icon(status: &BatteryStatus) -> Result<Icon> {
    let mut rgba = vec![0_u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    fill_background(&mut rgba);

    let left_text = format_line("L", status.left);
    let right_text = format_line("R", status.right);
    let left_color = level_color(status.left);
    let right_color = level_color(status.right);

    // 3x5 glyph scaled by 3 keeps two lines readable even in a tiny tray icon.
    draw_text_3x5(&mut rgba, ICON_SIZE, 2, 8, 3, &left_text, left_color);
    draw_text_3x5(&mut rgba, ICON_SIZE, 2, 36, 3, &right_text, right_color);

    Ok(Icon::from_rgba(rgba, ICON_SIZE, ICON_SIZE)?)
}

fn fill_background(buffer: &mut [u8]) {
    for y in 0..ICON_SIZE {
        for x in 0..ICON_SIZE {
            paint_pixel(buffer, ICON_SIZE, x, y, BG);
        }
    }
}

fn format_line(label: &str, level: Option<u8>) -> String {
    match level {
        Some(value) => format!("{label}:{value:>3}"),
        None => format!("{label}:  ?"),
    }
}

fn level_color(level: Option<u8>) -> [u8; 4] {
    match level {
        Some(50..=100) => WHITE,
        Some(20..=49) => YELLOW,
        Some(_) => RED,
        None => GRAY,
    }
}

fn draw_text_3x5(
    buffer: &mut [u8],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    scale: u32,
    text: &str,
    color: [u8; 4],
) {
    let mut cursor_x = origin_x;
    for ch in text.chars() {
        draw_glyph_3x5(buffer, width, cursor_x, origin_y, scale, ch, color);
        cursor_x += (3 + 1) * scale;
    }
}

fn draw_glyph_3x5(
    buffer: &mut [u8],
    width: u32,
    origin_x: u32,
    origin_y: u32,
    scale: u32,
    ch: char,
    color: [u8; 4],
) {
    let pattern = glyph_3x5(ch);
    for (row, mask) in pattern.iter().enumerate() {
        for col in 0..3_u32 {
            if (mask & (1 << (2 - col))) == 0 {
                continue;
            }
            let px = origin_x + col * scale;
            let py = origin_y + row as u32 * scale;
            fill_rect(buffer, width, px, py, scale, scale, color);
        }
    }
}

fn glyph_3x5(ch: char) -> [u8; 5] {
    match ch {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        'L' => [0b100, 0b100, 0b100, 0b100, 0b111],
        'R' => [0b110, 0b101, 0b110, 0b101, 0b101],
        ':' => [0b000, 0b010, 0b000, 0b010, 0b000],
        '?' => [0b111, 0b001, 0b011, 0b000, 0b010],
        ' ' => [0b000, 0b000, 0b000, 0b000, 0b000],
        _ => [0b111, 0b111, 0b111, 0b111, 0b111],
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
