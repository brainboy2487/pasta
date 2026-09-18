// src/stdlib/graphics/draw.rs
//! High-performance 2D drawing primitives for the Pasta graphics system.
//! All functions operate on a raw pixel buffer (width, height, &mut Vec<u32>)
//! using packed 0xAARRGGBB format, so the caller never needs to iterate pixels.

use crate::stdlib::graphics::canvas::Canvas;

// ── Line ─────────────────────────────────────────────────────────────────────

/// Bresenham integer line.
pub fn draw_line(canvas: &mut Canvas, x0: isize, y0: isize, x1: isize, y1: isize, color: u32) {
    let mut x0 = x0;
    let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1isize } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1isize } else { -1 };
    let mut err = dx + dy;
    loop {
        canvas.set_pixel(x0, y0, color);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

// ── Rectangle ────────────────────────────────────────────────────────────────

pub fn draw_rect(canvas: &mut Canvas, x: isize, y: isize, w: isize, h: isize, color: u32) {
    draw_line(canvas, x, y, x + w - 1, y, color);
    draw_line(canvas, x, y + h - 1, x + w - 1, y + h - 1, color);
    draw_line(canvas, x, y, x, y + h - 1, color);
    draw_line(canvas, x + w - 1, y, x + w - 1, y + h - 1, color);
}

pub fn fill_rect(canvas: &mut Canvas, x: isize, y: isize, w: isize, h: isize, color: u32) {
    if w <= 0 || h <= 0 {
        return;
    }
    canvas.fill_rect_region(x, y, w as usize, h as usize, color);
}

pub fn fill_grid_cell(
    canvas: &mut Canvas,
    cell_width: usize,
    cell_height: usize,
    grid_x: isize,
    grid_y: isize,
    color: u32,
) {
    if cell_width == 0 || cell_height == 0 {
        return;
    }
    let x = grid_x.saturating_mul(cell_width as isize);
    let y = grid_y.saturating_mul(cell_height as isize);
    canvas.fill_rect_region(x, y, cell_width, cell_height, color);
}

pub fn fill_grid_cells(
    canvas: &mut Canvas,
    cell_width: usize,
    cell_height: usize,
    cells: &[(isize, isize, u32)],
) {
    for &(grid_x, grid_y, color) in cells {
        fill_grid_cell(canvas, cell_width, cell_height, grid_x, grid_y, color);
    }
}

pub fn fill_grid_run(
    canvas: &mut Canvas,
    cell_width: usize,
    cell_height: usize,
    grid_x: isize,
    grid_y: isize,
    run_len: usize,
    color: u32,
) {
    if cell_width == 0 || cell_height == 0 || run_len == 0 {
        return;
    }
    let x = grid_x.saturating_mul(cell_width as isize);
    let y = grid_y.saturating_mul(cell_height as isize);
    let width = cell_width.saturating_mul(run_len);
    canvas.fill_rect_region(x, y, width, cell_height, color);
}

pub fn fill_grid_runs(
    canvas: &mut Canvas,
    cell_width: usize,
    cell_height: usize,
    runs: &[(isize, isize, usize, u32)],
) {
    for &(grid_x, grid_y, run_len, color) in runs {
        fill_grid_run(
            canvas,
            cell_width,
            cell_height,
            grid_x,
            grid_y,
            run_len,
            color,
        );
    }
}

// ── Circle ───────────────────────────────────────────────────────────────────

pub fn draw_circle(canvas: &mut Canvas, cx: isize, cy: isize, radius: isize, color: u32) {
    let mut x = radius;
    let mut y = 0isize;
    let mut err = 0isize;
    while x >= y {
        canvas.set_pixel(cx + x, cy + y, color);
        canvas.set_pixel(cx + y, cy + x, color);
        canvas.set_pixel(cx - y, cy + x, color);
        canvas.set_pixel(cx - x, cy + y, color);
        canvas.set_pixel(cx - x, cy - y, color);
        canvas.set_pixel(cx - y, cy - x, color);
        canvas.set_pixel(cx + y, cy - x, color);
        canvas.set_pixel(cx + x, cy - y, color);
        y += 1;
        if err <= 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

pub fn fill_circle(canvas: &mut Canvas, cx: isize, cy: isize, radius: isize, color: u32) {
    let mut x = radius;
    let mut y = 0isize;
    let mut err = 0isize;
    while x >= y {
        hline(canvas, cx - x, cx + x, cy + y, color);
        hline(canvas, cx - x, cx + x, cy - y, color);
        hline(canvas, cx - y, cx + y, cy + x, color);
        hline(canvas, cx - y, cx + y, cy - x, color);
        y += 1;
        if err <= 0 {
            err += 2 * y + 1;
        } else {
            x -= 1;
            err += 2 * (y - x) + 1;
        }
    }
}

// ── Ellipse ──────────────────────────────────────────────────────────────────

pub fn draw_ellipse(canvas: &mut Canvas, cx: isize, cy: isize, rx: isize, ry: isize, color: u32) {
    if rx == 0 || ry == 0 {
        return;
    }
    let mut x = 0isize;
    let mut y = ry;
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let mut d1 = ry2 - rx2 * ry + rx2 / 4;
    let mut dx = 2 * ry2 * x;
    let mut dy = 2 * rx2 * y;
    while dx < dy {
        plot4(canvas, cx, cy, x, y, color);
        x += 1;
        dx += 2 * ry2;
        if d1 < 0 {
            d1 += dx + ry2;
        } else {
            y -= 1;
            dy -= 2 * rx2;
            d1 += dx - dy + ry2;
        }
    }
    let mut d2 = ry2 * (x * x + x) + rx2 * (y * y - y) - rx2 * ry2 + rx2 / 4;
    while y >= 0 {
        plot4(canvas, cx, cy, x, y, color);
        y -= 1;
        dy -= 2 * rx2;
        if d2 > 0 {
            d2 += rx2 - dy;
        } else {
            x += 1;
            dx += 2 * ry2;
            d2 += dx - dy + rx2;
        }
    }
}

pub fn fill_ellipse(canvas: &mut Canvas, cx: isize, cy: isize, rx: isize, ry: isize, color: u32) {
    if rx == 0 || ry == 0 {
        return;
    }
    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let mut x = 0isize;
    let mut y = ry;
    let mut d1 = ry2 - rx2 * ry + rx2 / 4;
    let mut dx = 2 * ry2 * x;
    let mut dy = 2 * rx2 * y;
    while dx < dy {
        hline(canvas, cx - x, cx + x, cy + y, color);
        hline(canvas, cx - x, cx + x, cy - y, color);
        x += 1;
        dx += 2 * ry2;
        if d1 < 0 {
            d1 += dx + ry2;
        } else {
            y -= 1;
            dy -= 2 * rx2;
            d1 += dx - dy + ry2;
        }
    }
    let mut d2 = ry2 * (x * x + x) + rx2 * (y * y - y) - rx2 * ry2 + rx2 / 4;
    while y >= 0 {
        hline(canvas, cx - x, cx + x, cy + y, color);
        hline(canvas, cx - x, cx + x, cy - y, color);
        y -= 1;
        dy -= 2 * rx2;
        if d2 > 0 {
            d2 += rx2 - dy;
        } else {
            x += 1;
            dx += 2 * ry2;
            d2 += dx - dy + rx2;
        }
    }
}

// ── Triangle ─────────────────────────────────────────────────────────────────

pub fn draw_triangle(
    canvas: &mut Canvas,
    x0: isize,
    y0: isize,
    x1: isize,
    y1: isize,
    x2: isize,
    y2: isize,
    color: u32,
) {
    draw_line(canvas, x0, y0, x1, y1, color);
    draw_line(canvas, x1, y1, x2, y2, color);
    draw_line(canvas, x2, y2, x0, y0, color);
}

pub fn fill_triangle(
    canvas: &mut Canvas,
    mut x0: isize,
    mut y0: isize,
    mut x1: isize,
    mut y1: isize,
    mut x2: isize,
    mut y2: isize,
    color: u32,
) {
    // Sort vertices by Y
    if y0 > y1 {
        std::mem::swap(&mut x0, &mut x1);
        std::mem::swap(&mut y0, &mut y1);
    }
    if y0 > y2 {
        std::mem::swap(&mut x0, &mut x2);
        std::mem::swap(&mut y0, &mut y2);
    }
    if y1 > y2 {
        std::mem::swap(&mut x1, &mut x2);
        std::mem::swap(&mut y1, &mut y2);
    }

    let total_h = y2 - y0;
    if total_h == 0 {
        return;
    }
    for i in 0..total_h {
        let second_half = i > y1 - y0 || y1 == y0;
        let seg_h = if second_half { y2 - y1 } else { y1 - y0 };
        if seg_h == 0 {
            continue;
        }
        let alpha = i as f32 / total_h as f32;
        let beta = if second_half {
            (i - (y1 - y0)) as f32 / seg_h as f32
        } else {
            i as f32 / seg_h as f32
        };
        let mut ax = x0 + ((x2 - x0) as f32 * alpha) as isize;
        let mut bx = if second_half {
            x1 + ((x2 - x1) as f32 * beta) as isize
        } else {
            x0 + ((x1 - x0) as f32 * beta) as isize
        };
        if ax > bx {
            std::mem::swap(&mut ax, &mut bx);
        }
        hline(canvas, ax, bx, y0 + i, color);
    }
}

// ── Polygon ──────────────────────────────────────────────────────────────────

/// Draw polygon outline from a flat list of (x, y) pairs.
pub fn draw_polygon(canvas: &mut Canvas, points: &[(isize, isize)], color: u32) {
    if points.len() < 2 {
        return;
    }
    for i in 0..points.len() {
        let (x0, y0) = points[i];
        let (x1, y1) = points[(i + 1) % points.len()];
        draw_line(canvas, x0, y0, x1, y1, color);
    }
}

/// Fill convex polygon using scanline (works correctly for convex shapes).
pub fn fill_polygon(canvas: &mut Canvas, points: &[(isize, isize)], color: u32) {
    if points.len() < 3 {
        return;
    }
    let min_y = points.iter().map(|(_, y)| *y).min().unwrap();
    let max_y = points.iter().map(|(_, y)| *y).max().unwrap();
    let n = points.len();
    for y in min_y..=max_y {
        let mut intersections = Vec::new();
        for i in 0..n {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % n];
            if (y0 <= y && y < y1) || (y1 <= y && y < y0) {
                let x = x0 + (y - y0) * (x1 - x0) / (y1 - y0);
                intersections.push(x);
            }
        }
        intersections.sort_unstable();
        let mut k = 0;
        while k + 1 < intersections.len() {
            hline(canvas, intersections[k], intersections[k + 1], y, color);
            k += 2;
        }
    }
}

// ── Arc ──────────────────────────────────────────────────────────────────────

/// Draw a circular arc from start_deg to end_deg (degrees, 0=right, CCW).
pub fn draw_arc(
    canvas: &mut Canvas,
    cx: isize,
    cy: isize,
    radius: isize,
    start_deg: f64,
    end_deg: f64,
    color: u32,
) {
    if radius <= 0 {
        return;
    }
    let steps = (radius * 6).max(64) as usize;
    let start = start_deg.to_radians();
    let end = end_deg.to_radians();
    let step = (end - start) / steps as f64;
    let mut prev_x = cx + (start.cos() * radius as f64) as isize;
    let mut prev_y = cy - (start.sin() * radius as f64) as isize;
    for i in 1..=steps {
        let angle = start + step * i as f64;
        let nx = cx + (angle.cos() * radius as f64) as isize;
        let ny = cy - (angle.sin() * radius as f64) as isize;
        draw_line(canvas, prev_x, prev_y, nx, ny, color);
        prev_x = nx;
        prev_y = ny;
    }
}

// ── Color helpers ─────────────────────────────────────────────────────────────

/// Pack RGB (0–255 each) into 0xFFRRGGBB.
#[inline]
pub fn color_rgb(r: u8, g: u8, b: u8) -> u32 {
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Pack RGBA (0–255 each) into 0xAARRGGBB.
#[inline]
pub fn color_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Convert HSV (h: 0–360, s: 0–1, v: 0–1) to packed 0xFFRRGGBB.
pub fn color_hsv(h: f64, s: f64, v: f64) -> u32 {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    color_rgb(
        ((r1 + m) * 255.0).round() as u8,
        ((g1 + m) * 255.0).round() as u8,
        ((b1 + m) * 255.0).round() as u8,
    )
}

/// Linear interpolate between two packed colors. t = 0.0 → c1, t = 1.0 → c2.
pub fn color_lerp(c1: u32, c2: u32, t: f64) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let lerp_ch = |a: u32, b: u32| -> u32 { (a as f64 + (b as f64 - a as f64) * t).round() as u32 };
    let r = lerp_ch((c1 >> 16) & 0xFF, (c2 >> 16) & 0xFF);
    let g = lerp_ch((c1 >> 8) & 0xFF, (c2 >> 8) & 0xFF);
    let b = lerp_ch(c1 & 0xFF, c2 & 0xFF);
    let a = lerp_ch((c1 >> 24) & 0xFF, (c2 >> 24) & 0xFF);
    (a << 24) | (r << 16) | (g << 8) | b
}

// ── 16-bit (RGB565) color support ────────────────────────────────────────────

/// Build a 32-bit color from RGB565 components (r: 0-31, g: 0-63, b: 0-31).
/// Each component is expanded to 8 bits by shifting left (r<<3, g<<2, b<<3).
#[inline]
pub fn color_rgb16(r5: u8, g6: u8, b5: u8) -> u32 {
    let r = ((r5 & 0x1F) as u32) << 3;
    let g = ((g6 & 0x3F) as u32) << 2;
    let b = ((b5 & 0x1F) as u32) << 3;
    0xFF000000 | (r << 16) | (g << 8) | b
}

/// Expand a 16-bit RGB565 packed value (0–65535) to 0xFFRRGGBB.
#[inline]
pub fn color_from565(packed: u16) -> u32 {
    let r5 = ((packed >> 11) & 0x1F) as u8;
    let g6 = ((packed >> 5) & 0x3F) as u8;
    let b5 = (packed & 0x1F) as u8;
    color_rgb16(r5, g6, b5)
}

/// Compress a 32-bit 0xFFRRGGBB color down to a 16-bit RGB565 packed value.
/// Precision loss: R/B truncated to 5 bits, G to 6 bits.
#[inline]
pub fn color_to565(packed: u32) -> u16 {
    let r = ((packed >> 16) & 0xFF) as u16;
    let g = ((packed >> 8) & 0xFF) as u16;
    let b = (packed & 0xFF) as u16;
    ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
}

/// Return a fully-saturated rainbow color at `angle` degrees (0–360).
/// Equivalent to `color_hsv(angle, 1.0, 1.0)` but faster for rainbow sweeps.
#[inline]
pub fn color_wheel(angle: f64) -> u32 {
    color_hsv(angle % 360.0, 1.0, 1.0)
}

/// Return the number of distinct colors in the RGB565 palette (65,536).
#[inline]
pub const fn palette565_size() -> u32 {
    65_536
}

// ── Named 32-color palette ────────────────────────────────────────────────────

pub const COLOR_BLACK: u32 = 0xFF000000;
pub const COLOR_WHITE: u32 = 0xFFFFFFFF;
pub const COLOR_RED: u32 = 0xFFFF0000;
pub const COLOR_GREEN: u32 = 0xFF00FF00;
pub const COLOR_BLUE: u32 = 0xFF0000FF;
pub const COLOR_YELLOW: u32 = 0xFFFFFF00;
pub const COLOR_CYAN: u32 = 0xFF00FFFF;
pub const COLOR_MAGENTA: u32 = 0xFFFF00FF;
pub const COLOR_ORANGE: u32 = 0xFFFF8000;
pub const COLOR_PURPLE: u32 = 0xFF800080;
pub const COLOR_PINK: u32 = 0xFFFF69B4;
pub const COLOR_BROWN: u32 = 0xFF8B4513;
pub const COLOR_GRAY: u32 = 0xFF808080;
pub const COLOR_GREY: u32 = 0xFF808080;
pub const COLOR_DARK_GRAY: u32 = 0xFF404040;
pub const COLOR_LIGHT_GRAY: u32 = 0xFFC0C0C0;
pub const COLOR_DARK_RED: u32 = 0xFF8B0000;
pub const COLOR_DARK_GREEN: u32 = 0xFF006400;
pub const COLOR_DARK_BLUE: u32 = 0xFF00008B;
pub const COLOR_TEAL: u32 = 0xFF008080;
pub const COLOR_NAVY: u32 = 0xFF000080;
pub const COLOR_MAROON: u32 = 0xFF800000;
pub const COLOR_OLIVE: u32 = 0xFF808000;
pub const COLOR_LIME: u32 = 0xFF32CD32;
pub const COLOR_INDIGO: u32 = 0xFF4B0082;
pub const COLOR_VIOLET: u32 = 0xFFEE82EE;
pub const COLOR_GOLD: u32 = 0xFFFFD700;
pub const COLOR_SILVER: u32 = 0xFFC0C0C0;
pub const COLOR_CORAL: u32 = 0xFFFF7F50;
pub const COLOR_SALMON: u32 = 0xFFFA8072;
pub const COLOR_TURQUOISE: u32 = 0xFF40E0D0;
pub const COLOR_CRIMSON: u32 = 0xFFDC143C;
pub const COLOR_AZURE: u32 = 0xFFF0FFFF;

/// Register all 32 named colors as global number constants in the executor.
pub fn register_color_palette(env: &mut crate::interpreter::environment::Environment) {
    let palette: &[(&str, u32)] = &[
        ("COLOR_BLACK", COLOR_BLACK),
        ("COLOR_WHITE", COLOR_WHITE),
        ("COLOR_RED", COLOR_RED),
        ("COLOR_GREEN", COLOR_GREEN),
        ("COLOR_BLUE", COLOR_BLUE),
        ("COLOR_YELLOW", COLOR_YELLOW),
        ("COLOR_CYAN", COLOR_CYAN),
        ("COLOR_MAGENTA", COLOR_MAGENTA),
        ("COLOR_ORANGE", COLOR_ORANGE),
        ("COLOR_PURPLE", COLOR_PURPLE),
        ("COLOR_PINK", COLOR_PINK),
        ("COLOR_BROWN", COLOR_BROWN),
        ("COLOR_GRAY", COLOR_GRAY),
        ("COLOR_GREY", COLOR_GREY),
        ("COLOR_DARK_GRAY", COLOR_DARK_GRAY),
        ("COLOR_LIGHT_GRAY", COLOR_LIGHT_GRAY),
        ("COLOR_DARK_RED", COLOR_DARK_RED),
        ("COLOR_DARK_GREEN", COLOR_DARK_GREEN),
        ("COLOR_DARK_BLUE", COLOR_DARK_BLUE),
        ("COLOR_TEAL", COLOR_TEAL),
        ("COLOR_NAVY", COLOR_NAVY),
        ("COLOR_MAROON", COLOR_MAROON),
        ("COLOR_OLIVE", COLOR_OLIVE),
        ("COLOR_LIME", COLOR_LIME),
        ("COLOR_INDIGO", COLOR_INDIGO),
        ("COLOR_VIOLET", COLOR_VIOLET),
        ("COLOR_GOLD", COLOR_GOLD),
        ("COLOR_SILVER", COLOR_SILVER),
        ("COLOR_CORAL", COLOR_CORAL),
        ("COLOR_SALMON", COLOR_SALMON),
        ("COLOR_TURQUOISE", COLOR_TURQUOISE),
        ("COLOR_CRIMSON", COLOR_CRIMSON),
        ("COLOR_AZURE", COLOR_AZURE),
    ];
    for (name, val) in palette {
        env.set_global(
            *name,
            crate::interpreter::environment::Value::Number(*val as f64),
        );
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

#[inline]
fn hline(canvas: &mut Canvas, x0: isize, x1: isize, y: isize, color: u32) {
    let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
    for x in x0..=x1 {
        canvas.set_pixel(x, y, color);
    }
}

#[inline]
fn plot4(canvas: &mut Canvas, cx: isize, cy: isize, x: isize, y: isize, color: u32) {
    canvas.set_pixel(cx + x, cy + y, color);
    canvas.set_pixel(cx - x, cy + y, color);
    canvas.set_pixel(cx + x, cy - y, color);
    canvas.set_pixel(cx - x, cy - y, color);
}
