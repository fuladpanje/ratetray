const W: u32 = 32;
const H: u32 = 32;
const GW: u32 = 4;
const GH: u32 = 6;

// 4x6 pixel font for digits and dash. Each row is 4 bits, bit3 = leftmost.
const GLYPHS: &[[u16; 6]] = &[
    [0b0110, 0b1001, 0b1001, 0b1001, 0b1001, 0b0110], // 0
    [0b0010, 0b0110, 0b0010, 0b0010, 0b0010, 0b0111], // 1
    [0b1110, 0b0001, 0b0001, 0b0110, 0b1000, 0b1111], // 2
    [0b1110, 0b0001, 0b0001, 0b0110, 0b0001, 0b1110], // 3
    [0b1001, 0b1001, 0b1001, 0b1111, 0b0001, 0b0001], // 4
    [0b1111, 0b1000, 0b1110, 0b0001, 0b0001, 0b1110], // 5
    [0b0110, 0b1000, 0b1110, 0b1001, 0b1001, 0b0110], // 6
    [0b1111, 0b0001, 0b0010, 0b0100, 0b0100, 0b0100], // 7
    [0b0110, 0b1001, 0b0110, 0b1001, 0b1001, 0b0110], // 8
    [0b0110, 0b1001, 0b1001, 0b0111, 0b0001, 0b0110], // 9
];

const DASH: [u16; 6] = [0, 0, 0, 0b1111, 0, 0];

const GLYPH_M: [u16; 6] = [
    0b1001, 0b1111, 0b1111, 0b1001, 0b1001, 0b1001,
]; // M (million suffix)

const GLYPH_K: [u16; 6] = [
    0b1001, 0b1010, 0b1100, 0b1010, 0b1001, 0b1001,
]; // K (thousand-dollars suffix)

const GLYPH_DOT: [u16; 6] = [0, 0, 0, 0, 0b0110, 0b0110]; // decimal point

/// Splits the digits into rows that fit the 32x32 canvas.
/// Scale 2 is used (it halves exactly to the 16px tray size, staying crisp;
/// odd scales blur when Windows downscales the icon).
/// Up to 3 digits stay on a single row; 4 digits become 2+2 (e.g. "1020" -> "10" / "20").
fn layout(n: usize) -> (Vec<usize>, u32) {
    match n {
        0..=3 => (vec![n.max(1)], 2),
        4 => (vec![2, 2], 2),
        5 => (vec![3, 2], 2),
        6 => (vec![3, 3], 2),
        _ => (vec![(n + 1) / 2, n / 2], 1),
    }
}

/// Picks a digit color that contrasts with the taskbar background,
/// like the Windows clock does (white on dark taskbar, black on light).
fn digit_color() -> (u8, u8, u8) {
    #[cfg(windows)]
    if let Some(luma) = sample_taskbar_luma() {
        return if luma > 140.0 { (0, 0, 0) } else { (255, 255, 255) };
    }
    (255, 255, 255)
}

#[cfg(windows)]
fn sample_taskbar_luma() -> Option<f64> {
    use std::os::raw::{c_int, c_void};

    #[link(name = "user32")]
    extern "system" {
        fn GetDC(hdc: *mut c_void) -> *mut c_void;
        fn ReleaseDC(hdc: *mut c_void, h: *mut c_void) -> c_int;
        fn GetPixel(hdc: *mut c_void, x: c_int, y: c_int) -> u32;
        fn GetSystemMetrics(idx: c_int) -> c_int;
    }

    unsafe {
        let w = GetSystemMetrics(0);
        let h = GetSystemMetrics(1);
        if w <= 8 || h <= 8 {
            return None;
        }
        let hdc = GetDC(std::ptr::null_mut());
        if hdc.is_null() {
            return None;
        }
        // Far-right edge of the taskbar (the "show desktop" sliver) = taskbar background.
        let pts = [(w - 2, h - 6), (w - 2, h - 16), (w - 2, h - 28), (w - 3, h - 12)];
        let mut sum = 0.0;
        let mut count = 0;
        for (x, y) in pts {
            let c = GetPixel(hdc, x, y);
            let r = (c & 0xFF) as f64;
            let g = ((c >> 8) & 0xFF) as f64;
            let b = ((c >> 16) & 0xFF) as f64;
            sum += 0.299 * r + 0.587 * g + 0.114 * b;
            count += 1;
        }
        ReleaseDC(std::ptr::null_mut(), hdc);
        if count == 0 {
            None
        } else {
            Some(sum / count as f64)
        }
    }
}

/// Renders `text` as a transparent-background tray icon (like the Windows clock).
pub fn render(text: &str) -> Vec<u8> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len().max(1);
    let (rows, scale) = layout(n);

    let (fr, fg, fb) = digit_color();

    let mut buf = vec![0u8; (W * H * 4) as usize];

    let row_h = GH * scale;
    let gap_v = scale;
    let total_h = rows.len() as u32 * row_h + (rows.len() as u32 - 1) * gap_v;
    let y_start = H.saturating_sub(total_h) / 2;

    let mut offset = 0;
    for (ri, &row_len) in rows.iter().enumerate() {
        let gap: u32 = if row_len == 2 {
            // Gap 2 keeps digits grouped; even geometry stays crisp when
            // Windows downscales the icon (odd offsets blur).
            2
        } else if GW * row_len as u32 + (row_len as u32 - 1) > W / scale {
            0
        } else {
            1
        };
        let text_w = (GW * row_len as u32 + gap * (row_len as u32 - 1)) * scale;
        let x0 = (W.saturating_sub(text_w)) / 2;
        let y0 = y_start + ri as u32 * (row_h + gap_v);

        for i in 0..row_len {
            let ch = chars.get(offset + i).copied().unwrap_or('-');
            let glyph = match ch.to_digit(10) {
                Some(d) => &GLYPHS[d as usize],
                None if ch == 'M' || ch == 'm' => &GLYPH_M,
                None if ch == 'K' || ch == 'k' => &GLYPH_K,
                None if ch == '.' => &GLYPH_DOT,
                None => &DASH,
            };
            let gx = x0 + i as u32 * (GW + gap) * scale;
            for row in 0..GH {
                for col in 0..GW {
                    if glyph[row as usize] & (1 << (GW - 1 - col)) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = gx + col * scale + sx;
                            let py = y0 + row * scale + sy;
                            if px >= W || py >= H {
                                continue;
                            }
                            let idx = ((py * W + px) * 4) as usize;
                            buf[idx] = fr;
                            buf[idx + 1] = fg;
                            buf[idx + 2] = fb;
                            buf[idx + 3] = 255;
                        }
                    }
                }
            }
        }
        offset += row_len;
    }

    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opaque(buf: &[u8], x: u32, y: u32) -> bool {
        buf[((y * W + x) * 4 + 3) as usize] == 255
    }

    fn band_has_pixels(buf: &[u8], y0: u32, y1: u32) -> bool {
        (y0..y1).any(|y| (0..W).any(|x| opaque(buf, x, y)))
    }

    #[test]
    fn three_digits_single_row() {
        let buf = render("254");
        // Single 12px-high row, vertically centered: y in [10, 22)
        assert!(band_has_pixels(&buf, 10, 22), "digits missing from center row");
        assert!(!band_has_pixels(&buf, 0, 10), "unexpected pixels above");
        assert!(!band_has_pixels(&buf, 22, 32), "unexpected pixels below");
    }

    #[test]
    fn four_digits_two_rows() {
        let buf = render("1020");
        // Two 12px rows with a 2px gap: [3,15) / gap [15,17) / [17,29)
        assert!(band_has_pixels(&buf, 3, 15), "top row missing");
        assert!(band_has_pixels(&buf, 17, 29), "bottom row missing");
        assert!(!band_has_pixels(&buf, 15, 17), "gap between rows must stay empty");
        assert!(!band_has_pixels(&buf, 0, 3), "unexpected pixels above");
        assert!(!band_has_pixels(&buf, 29, 32), "unexpected pixels below");
    }

    #[test]
    fn m_suffix_single_row() {
        let buf = render("25M");
        // 3 chars -> single centered row, M glyph must draw pixels.
        assert!(band_has_pixels(&buf, 10, 22), "digits missing from center row");
        assert!(!band_has_pixels(&buf, 0, 10), "unexpected pixels above");
        assert!(!band_has_pixels(&buf, 22, 32), "unexpected pixels below");
    }

    #[test]
    fn two_digits_fill_width() {
        let buf = render("69");
        // Tight even geometry: x0 = 6 (even), so downscaling stays crisp.
        assert!(band_has_pixels(&buf, 10, 22), "digits missing");
        let mut min_x = W;
        let mut max_x = 0;
        for y in 10..22 {
            for x in 0..W {
                if opaque(&buf, x, y) {
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                }
            }
        }
        assert!(min_x <= 7 && max_x >= 24, "digits should span the icon width");
    }

    #[test]
    fn decimal_point_renders() {
        let buf = render("5.2");
        // 3 chars -> scale 2 single row [10, 22); dot at x 14..17, y 18..21.
        assert!(band_has_pixels(&buf, 10, 22), "digits missing");
        assert!(opaque(&buf, 15, 19), "decimal dot missing");
    }

    #[test]
    fn k_suffix_single_row() {
        let buf = render("84K");
        assert!(band_has_pixels(&buf, 10, 22), "glyphs missing");
        assert!(!band_has_pixels(&buf, 0, 10), "unexpected pixels above");
        assert!(!band_has_pixels(&buf, 22, 32), "unexpected pixels below");
    }

    #[test]
    fn text_origins_stay_even() {
        // Even x origins keep edges crisp when Windows downscales 32 -> 16.
        for text in ["254", "25M", "5.2", "69", "--"] {
            let buf = render(text);
            let mut min_x = W;
            for y in 0..H {
                for x in 0..W {
                    if opaque(&buf, x, y) {
                        min_x = min_x.min(x);
                    }
                }
            }
            assert_eq!(min_x % 2, 0, "{text}: odd origin blurs on downscale");
        }
    }

    #[test]
    fn background_is_transparent() {
        let buf = render("1020");
        // Corners are never part of a glyph -> must be fully transparent.
        for &(x, y) in &[(0, 0), (31, 0), (0, 31), (31, 31)] {
            assert_eq!(buf[((y * W + x) * 4 + 3) as usize], 0, "corner ({x},{y}) not transparent");
        }
        // No dark rounded rectangle behind the digits: any opaque pixel must be a digit pixel
        // (i.e. never a full-frame fill).
        let opaque_count = (0..H)
            .map(|y| (0..W).filter(|&x| opaque(&buf, x, y)).count())
            .sum::<usize>();
        assert!(opaque_count < (W * H) as usize / 2, "background fill detected");
    }

    #[test]
    fn five_and_six_digits_fit() {
        for text in ["10200", "102000"] {
            let buf = render(text);
            assert!(band_has_pixels(&buf, 3, 15), "{text}: top row missing");
            assert!(band_has_pixels(&buf, 17, 29), "{text}: bottom row missing");
        }
    }
}
