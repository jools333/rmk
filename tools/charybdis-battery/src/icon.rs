use crate::hid::{BatterySnapshot, ChargeState};

pub const ICON_SIZE: u32 = 32;

fn color_for_level(level: Option<u8>, charge: ChargeState, connected: bool) -> [u8; 4] {
    if !connected {
        return [120, 125, 130, 220];
    }
    if charge == ChargeState::Charging {
        return [80, 185, 255, 255]; // Cyan-blue for charging
    }
    match level {
        None => [130, 135, 140, 220],
        Some(pct) if pct >= 40 => [72, 199, 116, 255],  // Green
        Some(pct) if pct >= 20 => [245, 185, 66, 255],  // Amber
        Some(_) => [239, 83, 80, 255],                  // Red
    }
}

fn fill_rect(buf: &mut [u8], x0: u32, y0: u32, w: u32, h: u32, rgba: [u8; 4]) {
    for y in y0..(y0 + h).min(ICON_SIZE) {
        for x in x0..(x0 + w).min(ICON_SIZE) {
            let idx = ((y * ICON_SIZE + x) * 4) as usize;
            buf[idx..idx + 4].copy_from_slice(&rgba);
        }
    }
}

fn draw_battery_bar(
    buf: &mut [u8],
    x0: u32,
    level: Option<u8>,
    charge: ChargeState,
    connected: bool,
) {
    let border = [225, 230, 235, 255];
    let bg = [28, 32, 38, 220];

    // Battery cap (top nub)
    fill_rect(buf, x0 + 3, 2, 5, 2, border);
    // Outer frame (11 x 25)
    fill_rect(buf, x0, 4, 11, 25, border);
    // Inner background (7 x 21)
    fill_rect(buf, x0 + 2, 6, 7, 21, bg);

    if let Some(pct) = level {
        let fill_h = ((pct as u32 * 21) + 50) / 100;
        let fill_h = fill_h.clamp(1, 21);
        let fill_y = 6 + (21 - fill_h);
        let color = color_for_level(level, charge, connected);
        fill_rect(buf, x0 + 2, fill_y, 7, fill_h, color);
    } else if connected {
        // Unknown level indicator (middle dash)
        let color = color_for_level(None, charge, connected);
        fill_rect(buf, x0 + 3, 15, 5, 3, color);
    }
}

/// Render a 32x32 RGBA icon showing left and right half battery bars side-by-side.
pub fn render_rgba(snap: &BatterySnapshot) -> Vec<u8> {
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    let active = snap.connected && snap.link_connected;
    draw_battery_bar(&mut buf, 3, snap.left_level, snap.left_charge, active);
    draw_battery_bar(&mut buf, 18, snap.right_level, snap.right_charge, active);
    buf
}

/// Convert RGBA buffer to ARGB32 network byte order (used by Linux StatusNotifierItem `ksni`).
#[cfg(target_os = "linux")]
pub fn render_argb32(snap: &BatterySnapshot) -> Vec<u8> {
    let rgba = render_rgba(snap);
    let mut argb = Vec::with_capacity(rgba.len());
    for px in rgba.chunks_exact(4) {
        argb.push(px[3]); // A
        argb.push(px[0]); // R
        argb.push(px[1]); // G
        argb.push(px[2]); // B
    }
    argb
}
