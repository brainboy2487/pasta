// src/stdlib/graphics/canvas.rs
//! CPU pixel buffer (RGBA8) and SharedCanvas wrapper.

use std::sync::{Arc, Mutex};

pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u32>, // 0xAARRGGBB
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self { width, height, pixels: vec![0xFF000000u32; width * height] }
    }

    pub fn clear(&mut self, color: u32) { self.pixels.fill(color); }

    pub fn set_pixel(&mut self, x: isize, y: isize, color: u32) {
        if x < 0 || y < 0 { return; }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height { return; }
        self.pixels[y * self.width + x] = color;
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, self.pixels.len() * 4)
        }
    }
}

pub type SharedCanvas = Arc<Mutex<Canvas>>;

pub fn rgb_to_u32(r: u8, g: u8, b: u8) -> u32 {
    let a: u8 = 0xFF;
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Convert ARGB pixel buffer to BGRA byte layout expected by X11 XPutImage (32bpp).
/// XImage on little-endian x86 expects bytes as: B G R X (padding byte).
impl Canvas {
    pub fn as_bgra_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &px in &self.pixels {
            // px is 0xAARRGGBB
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >>  8) & 0xFF) as u8;
            let b = ( px        & 0xFF) as u8;
            out.push(b); // B
            out.push(g); // G
            out.push(r); // R
            out.push(0); // padding (X11 ignores alpha in ZPixmap 32bpp)
        }
        out
    }

    /// Fill the entire canvas with an RGB color.
    pub fn fill(&mut self, r: u8, g: u8, b: u8) {
        let color = rgb_to_u32(r, g, b);
        self.pixels.fill(color);
    }

    /// Copy from a raw RGB byte slice (3 bytes per pixel, row-major).
    pub fn load_rgb(&mut self, data: &[u8]) {
        for (i, chunk) in data.chunks(3).enumerate() {
            if i >= self.pixels.len() { break; }
            if chunk.len() < 3 { break; }
            self.pixels[i] = rgb_to_u32(chunk[0], chunk[1], chunk[2]);
        }
    }
}
