// src/stdlib/graphics/draw.rs
//! Bresenham line, filled rect, midpoint circle.

use crate::stdlib::graphics::canvas::Canvas;

pub fn draw_line(canvas: &mut Canvas, x0: isize, y0: isize, x1: isize, y1: isize, color: u32) {
    let mut x0 = x0; let mut y0 = y0;
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    loop {
        canvas.set_pixel(x0, y0, color);
        if x0 == x1 && y0 == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x0 += sx; }
        if e2 <= dx { err += dx; y0 += sy; }
    }
}

pub fn fill_rect(canvas: &mut Canvas, x: isize, y: isize, w: usize, h: usize, color: u32) {
    for yy in 0..(h as isize) { for xx in 0..(w as isize) { canvas.set_pixel(x + xx, y + yy, color); } }
}

pub fn draw_circle(canvas: &mut Canvas, cx: isize, cy: isize, radius: isize, color: u32) {
    let mut x = radius; let mut y = 0isize; let mut err = 0isize;
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
        if err <= 0 { err += 2*y + 1; } else { x -= 1; err += 2*(y - x) + 1; }
    }
}
