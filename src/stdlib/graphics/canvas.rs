// src/stdlib/graphics/canvas.rs
//! CPU pixel buffer (RGBA8) and SharedCanvas wrapper.

use std::sync::{Arc, Mutex};

/// In-memory ARGB canvas used by the graphics runtime.
#[derive(Clone)]
pub struct Canvas {
    /// Canvas width in pixels.
    pub width: usize,
    /// Canvas height in pixels.
    pub height: usize,
    /// Pixel buffer stored as `0xAARRGGBB`.
    pub pixels: Vec<u32>, // 0xAARRGGBB
}

impl Canvas {
    /// Create a new canvas initialized to opaque black.
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0xFF000000u32; width * height],
        }
    }

    /// Fill the entire canvas with a single color.
    pub fn clear(&mut self, color: u32) {
        self.pixels.fill(color);
    }

    /// Set one pixel if it falls within the canvas bounds.
    pub fn set_pixel(&mut self, x: isize, y: isize, color: u32) {
        if x < 0 || y < 0 {
            return;
        }
        let (x, y) = (x as usize, y as usize);
        if x >= self.width || y >= self.height {
            return;
        }
        self.pixels[y * self.width + x] = color;
    }

    /// Fast fill of a rectangular region with a single color.
    pub fn fill_rect_region(&mut self, x: isize, y: isize, w: usize, h: usize, color: u32) {
        if w == 0 || h == 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = (x + w as isize).min(self.width as isize) as usize;
        let y1 = (y + h as isize).min(self.height as isize) as usize;
        for yy in y0..y1 {
            let row_start = yy * self.width + x0;
            let row_end = yy * self.width + x1;
            if row_start < row_end && row_end <= self.pixels.len() {
                self.pixels[row_start..row_end].fill(color);
            }
        }
    }

    /// Copy a rectangular region from another canvas into this one.
    pub fn copy_region_from(
        &mut self,
        src: &Canvas,
        src_x: usize,
        src_y: usize,
        width: usize,
        height: usize,
        dst_x: usize,
        dst_y: usize,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        if src_x >= src.width || src_y >= src.height || dst_x >= self.width || dst_y >= self.height
        {
            return;
        }

        let copy_w = width.min(src.width - src_x).min(self.width - dst_x);
        let copy_h = height.min(src.height - src_y).min(self.height - dst_y);
        if copy_w == 0 || copy_h == 0 {
            return;
        }

        for row in 0..copy_h {
            let src_start = (src_y + row) * src.width + src_x;
            let src_end = src_start + copy_w;
            let dst_start = (dst_y + row) * self.width + dst_x;
            let dst_end = dst_start + copy_w;
            self.pixels[dst_start..dst_end].copy_from_slice(&src.pixels[src_start..src_end]);
        }
    }

    /// Return the color at one pixel, or `0` when out of bounds.
    pub fn get_pixel(&self, x: isize, y: isize) -> u32 {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return 0;
        }
        self.pixels[(y as usize) * self.width + x as usize]
    }

    /// Save the canvas as a binary PPM image.
    pub fn save_ppm(&self, path: &str) -> anyhow::Result<()> {
        use std::io::Write;
        let mut f = std::fs::File::create(path)?;
        write!(f, "P6\n{} {}\n255\n", self.width, self.height)?;
        for &pixel in &self.pixels {
            let r = ((pixel >> 16) & 0xFF) as u8;
            let g = ((pixel >> 8) & 0xFF) as u8;
            let b = (pixel & 0xFF) as u8;
            f.write_all(&[r, g, b])?;
        }
        Ok(())
    }

    /// View the canvas pixel buffer as raw bytes.
    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, self.pixels.len() * 4)
        }
    }
}

/// Thread-safe shared canvas handle.
pub type SharedCanvas = Arc<Mutex<Canvas>>;

/// Convert RGB components to an opaque ARGB color.
pub fn rgb_to_u32(r: u8, g: u8, b: u8) -> u32 {
    let a: u8 = 0xFF;
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Convert ARGB pixel buffer to BGRA byte layout expected by X11 XPutImage (32bpp).
/// XImage on little-endian x86 expects bytes as: B G R X (padding byte).
impl Canvas {
    /// Convert the ARGB pixel buffer into BGRA bytes for X11 upload paths.
    pub fn as_bgra_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &px in &self.pixels {
            // px is 0xAARRGGBB
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >> 8) & 0xFF) as u8;
            let b = (px & 0xFF) as u8;
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
        let mut non_zero = 0;
        for (i, chunk) in data.chunks(3).enumerate() {
            if i >= self.pixels.len() {
                break;
            }
            if chunk.len() < 3 {
                break;
            }
            if chunk[0] != 0 || chunk[1] != 0 || chunk[2] != 0 {
                non_zero += 1;
            }
            self.pixels[i] = rgb_to_u32(chunk[0], chunk[1], chunk[2]);
        }
        eprintln!(
            "[Canvas] load_rgb: {} bytes -> {} pixels, {} non-zero",
            data.len(),
            self.pixels.len(),
            non_zero
        );
    }
}
