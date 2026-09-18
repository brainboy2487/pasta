//! Pasta 2-Byte Compact Pixel Encoding (v1.0)
//!
//! A compact, flag-driven, metadata-rich pixel format for Pasta's graphics runtime.
//!
//! ## 16-bit Pixel Layout (RGB332 mode - default)
//! ```text
//! [ R R R G G G B B | X X X X | Y Y Y Y ]
//!        8 bits        4 bits    4 bits
//!      (RGB332)       (x-hint)  (y-hint)
//! ```
//!
//! RGB332 mode: 3 bits red (8 levels), 3 bits green (8 levels), 2 bits blue (4 levels)
//! This gives 256 distinct colors with reasonable color representation.
//!
//! Alternative: intensity mode uses 8-bit grayscale with base color scaling.
//!
//! This provides:
//! - 2x memory reduction vs 4-byte RGBA
//! - 256 colors in RGB332 mode
//! - Per-pixel spatial metadata for optimization

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Global base color for intensity scaling (0xRRGGBB format)
static BASE_COLOR: AtomicU32 = AtomicU32::new(0xFFFFFF); // Default: white

/// Color mode: true = RGB332, false = intensity/grayscale
static USE_RGB332: AtomicBool = AtomicBool::new(true); // Default: RGB332 for color support

/// 16-color default palette (can be extended to 256)
/// Index 0 = black, Index 255 = white in grayscale mode
pub static mut PALETTE: [u32; 256] = [0u32; 256];

/// Initialize the default grayscale palette (0=black to 255=white)
pub fn init_default_palette() {
    unsafe {
        for i in 0..256 {
            let v = i as u32;
            PALETTE[i] = (v << 16) | (v << 8) | v; // Grayscale: R=G=B=i
        }
    }
}

/// Set a palette entry (index 0-255, color as 0xRRGGBB)
pub fn palette_set(index: u8, color: u32) {
    unsafe {
        PALETTE[index as usize] = color & 0xFFFFFF;
    }
}

/// Get a palette entry
pub fn palette_get(index: u8) -> u32 {
    unsafe { PALETTE[index as usize] }
}

/// Enable RGB332 color mode (default)
pub fn set_rgb332_mode(enabled: bool) {
    USE_RGB332.store(enabled, Ordering::SeqCst);
}

/// Check if RGB332 mode is enabled
pub fn is_rgb332_mode() -> bool {
    USE_RGB332.load(Ordering::SeqCst)
}

/// Convert RGB to RGB332 format (3 bits R, 3 bits G, 2 bits B)
#[inline]
pub fn rgb_to_rgb332(r: u8, g: u8, b: u8) -> u8 {
    ((r >> 5) << 5) | ((g >> 5) << 2) | (b >> 6)
}

/// Convert RGB332 back to full RGB
#[inline]
pub fn rgb332_to_rgb(c: u8) -> (u8, u8, u8) {
    // Extract 3-bit R, 3-bit G, 2-bit B
    let r3 = (c >> 5) & 0x07;
    let g3 = (c >> 2) & 0x07;
    let b2 = c & 0x03;

    // Scale up: 3-bit (0-7) to 8-bit (0-255), 2-bit (0-3) to 8-bit
    // r3 * 255 / 7 ≈ r3 * 36.4 ≈ (r3 << 5) | (r3 << 2) | (r3 >> 1)
    let r = (r3 << 5) | (r3 << 2) | (r3 >> 1);
    let g = (g3 << 5) | (g3 << 2) | (g3 >> 1);
    let b = (b2 << 6) | (b2 << 4) | (b2 << 2) | b2;

    (r as u8, g as u8, b as u8)
}

/// Set the global base color for intensity scaling (0xRRGGBB)
pub fn set_base_color(color: u32) {
    BASE_COLOR.store(color & 0xFFFFFF, Ordering::SeqCst);
}

/// Get the global base color
pub fn get_base_color() -> u32 {
    BASE_COLOR.load(Ordering::SeqCst)
}

/// A 2-byte compact pixel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(transparent)]
pub struct CompactPixel(pub u16);

impl CompactPixel {
    /// Create a new compact pixel from color value and XY hints
    ///
    /// # Arguments
    /// * `color` - 8-bit color value (0-255)
    /// * `x_hint` - 4-bit X coordinate hint (0-15)
    /// * `y_hint` - 4-bit Y coordinate hint (0-15)
    #[inline]
    pub fn new(color: u8, x_hint: u8, y_hint: u8) -> Self {
        let x = (x_hint & 0x0F) as u16;
        let y = (y_hint & 0x0F) as u16;
        CompactPixel((color as u16) | (x << 8) | (y << 12))
    }

    /// Create a pixel with just color (x_hint and y_hint default to 0)
    #[inline]
    pub fn from_color(color: u8) -> Self {
        CompactPixel(color as u16)
    }

    /// Create from RGB values (converts to grayscale/palette index)
    /// Create from RGB values (uses RGB332 or grayscale depending on mode)
    #[inline]
    pub fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        let color = if is_rgb332_mode() {
            rgb_to_rgb332(r, g, b)
        } else {
            // Convert RGB to luminance (grayscale) using standard weights
            // Y = 0.299*R + 0.587*G + 0.114*B
            ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8
        };
        CompactPixel::from_color(color)
    }

    /// Create from RGB with coordinate hints
    #[inline]
    pub fn from_rgb_with_hints(r: u8, g: u8, b: u8, x_hint: u8, y_hint: u8) -> Self {
        let color = if is_rgb332_mode() {
            rgb_to_rgb332(r, g, b)
        } else {
            ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8
        };
        CompactPixel::new(color, x_hint, y_hint)
    }

    /// Extract the 8-bit color value
    #[inline]
    pub fn color(&self) -> u8 {
        (self.0 & 0xFF) as u8
    }

    /// Extract the 4-bit X hint
    #[inline]
    pub fn x_hint(&self) -> u8 {
        ((self.0 >> 8) & 0x0F) as u8
    }

    /// Extract the 4-bit Y hint
    #[inline]
    pub fn y_hint(&self) -> u8 {
        ((self.0 >> 12) & 0x0F) as u8
    }

    /// Decode to full 24-bit RGB (uses RGB332 or intensity mode)
    #[inline]
    pub fn to_rgb(&self) -> (u8, u8, u8) {
        if is_rgb332_mode() {
            rgb332_to_rgb(self.color())
        } else {
            self.to_rgb_intensity()
        }
    }

    /// Decode to full 24-bit RGB using intensity scaling
    /// Scales the global base color by the pixel's intensity (0-255)
    #[inline]
    pub fn to_rgb_intensity(&self) -> (u8, u8, u8) {
        let base = get_base_color();
        let intensity = self.color() as u32;

        let base_r = (base >> 16) & 0xFF;
        let base_g = (base >> 8) & 0xFF;
        let base_b = base & 0xFF;

        // Scale each channel by intensity/255
        let r = ((base_r * intensity) / 255) as u8;
        let g = ((base_g * intensity) / 255) as u8;
        let b = ((base_b * intensity) / 255) as u8;

        (r, g, b)
    }

    /// Decode to ARGB u32 (0xFFRRGGBB)
    #[inline]
    pub fn to_argb(&self) -> u32 {
        let (r, g, b) = self.to_rgb();
        0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
    }

    /// Decode to BGRA bytes for X11 (B, G, R, padding)
    #[inline]
    pub fn to_bgra_bytes(&self) -> [u8; 4] {
        let (r, g, b) = self.to_rgb();
        [b, g, r, 0]
    }
}

/// Compact Canvas using 2-byte pixels
#[derive(Clone)]
pub struct CompactCanvas {
    /// Canvas width in pixels.
    pub width: usize,
    /// Canvas height in pixels.
    pub height: usize,
    /// Packed RGB565 pixel buffer.
    pub pixels: Vec<u16>,
}

impl CompactCanvas {
    /// Create a new compact canvas initialized to black
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![0u16; width * height],
        }
    }

    /// Clear the canvas with a color value
    pub fn clear(&mut self, color: u8) {
        let pixel = CompactPixel::from_color(color).0;
        self.pixels.fill(pixel);
    }

    /// Set a pixel at (x, y)
    #[inline]
    pub fn set_pixel(&mut self, x: isize, y: isize, color: u8) {
        if x < 0 || y < 0 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux >= self.width || uy >= self.height {
            return;
        }
        let idx = uy * self.width + ux;
        // Encode XY hints as position % 16
        let x_hint = (ux & 0x0F) as u8;
        let y_hint = (uy & 0x0F) as u8;
        self.pixels[idx] = CompactPixel::new(color, x_hint, y_hint).0;
    }

    /// Set a pixel with RGB values (auto-converts to grayscale)
    #[inline]
    pub fn set_pixel_rgb(&mut self, x: isize, y: isize, r: u8, g: u8, b: u8) {
        if x < 0 || y < 0 {
            return;
        }
        let (ux, uy) = (x as usize, y as usize);
        if ux >= self.width || uy >= self.height {
            return;
        }
        let idx = uy * self.width + ux;
        let x_hint = (ux & 0x0F) as u8;
        let y_hint = (uy & 0x0F) as u8;
        self.pixels[idx] = CompactPixel::from_rgb_with_hints(r, g, b, x_hint, y_hint).0;
    }

    /// Get the pixel at (x, y)
    #[inline]
    pub fn get_pixel(&self, x: usize, y: usize) -> CompactPixel {
        if x >= self.width || y >= self.height {
            return CompactPixel::default();
        }
        CompactPixel(self.pixels[y * self.width + x])
    }

    /// Fill a rectangle with a color
    pub fn fill_rect(&mut self, x: isize, y: isize, w: usize, h: usize, color: u8) {
        if w == 0 || h == 0 {
            return;
        }
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w as isize) as usize).min(self.width);
        let y1 = ((y + h as isize) as usize).min(self.height);

        for yy in y0..y1 {
            for xx in x0..x1 {
                let idx = yy * self.width + xx;
                let x_hint = (xx & 0x0F) as u8;
                let y_hint = (yy & 0x0F) as u8;
                self.pixels[idx] = CompactPixel::new(color, x_hint, y_hint).0;
            }
        }
    }

    /// Fill a rectangle with RGB color (uses current color mode)
    pub fn fill_rect_rgb(&mut self, x: isize, y: isize, w: usize, h: usize, r: u8, g: u8, b: u8) {
        // Convert RGB to compact format (RGB332 or grayscale)
        let color = if is_rgb332_mode() {
            rgb_to_rgb332(r, g, b)
        } else {
            ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8
        };
        self.fill_rect(x, y, w, h, color);
    }

    /// Convert canvas to ARGB buffer for display (4 bytes per pixel)
    pub fn to_argb_buffer(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|&p| CompactPixel(p).to_argb())
            .collect()
    }

    /// Convert canvas to BGRA bytes for X11 display
    pub fn to_bgra_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.pixels.len() * 4);
        for &p in &self.pixels {
            let bytes = CompactPixel(p).to_bgra_bytes();
            out.extend_from_slice(&bytes);
        }
        out
    }

    /// Get raw pixel data as bytes (2 bytes per pixel)
    pub fn as_raw_bytes(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(self.pixels.as_ptr() as *const u8, self.pixels.len() * 2)
        }
    }

    /// Memory size in bytes (half of traditional RGBA canvas)
    pub fn memory_size(&self) -> usize {
        self.pixels.len() * 2
    }
}

// ============================================================================
// Hex color conversion utilities
// ============================================================================

/// Parse a hex color string (e.g., "#FF0000" or "FF0000") to RGB
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Convert RGB to 8-bit grayscale/intensity value
pub fn rgb_to_intensity(r: u8, g: u8, b: u8) -> u8 {
    ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8
}

/// Convert hex color to 8-bit intensity value
pub fn hex_to_intensity(hex: &str) -> Option<u8> {
    hex_to_rgb(hex).map(|(r, g, b)| rgb_to_intensity(r, g, b))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_encode_decode() {
        let pixel = CompactPixel::new(128, 5, 10);
        assert_eq!(pixel.color(), 128);
        assert_eq!(pixel.x_hint(), 5);
        assert_eq!(pixel.y_hint(), 10);
    }

    #[test]
    fn test_pixel_from_color() {
        let pixel = CompactPixel::from_color(255);
        assert_eq!(pixel.color(), 255);
        assert_eq!(pixel.x_hint(), 0);
        assert_eq!(pixel.y_hint(), 0);
    }

    #[test]
    fn test_round_trip() {
        for color in [0u8, 1, 127, 128, 254, 255] {
            for x in [0u8, 7, 15] {
                for y in [0u8, 7, 15] {
                    let pixel = CompactPixel::new(color, x, y);
                    assert_eq!(pixel.color(), color);
                    assert_eq!(pixel.x_hint(), x);
                    assert_eq!(pixel.y_hint(), y);
                }
            }
        }
    }

    #[test]
    fn test_grayscale_conversion() {
        // Test grayscale mode
        set_rgb332_mode(false);

        // Pure white
        let pixel = CompactPixel::from_rgb(255, 255, 255);
        assert_eq!(pixel.color(), 255);

        // Pure black
        let pixel = CompactPixel::from_rgb(0, 0, 0);
        assert_eq!(pixel.color(), 0);

        // Gray (127, 127, 127) should be ~127
        let pixel = CompactPixel::from_rgb(127, 127, 127);
        assert!(pixel.color() >= 126 && pixel.color() <= 128);

        // Reset to RGB332 mode
        set_rgb332_mode(true);
    }

    #[test]
    fn test_rgb332_conversion() {
        set_rgb332_mode(true);

        // Pure red (255, 0, 0) -> RGB332: 0b11100000 = 0xE0
        let pixel = CompactPixel::from_rgb(255, 0, 0);
        assert_eq!(pixel.color(), 0xE0, "Red should encode to 0xE0");

        // Pure green (0, 255, 0) -> RGB332: 0b00011100 = 0x1C
        let pixel = CompactPixel::from_rgb(0, 255, 0);
        assert_eq!(pixel.color(), 0x1C, "Green should encode to 0x1C");

        // Pure blue (0, 0, 255) -> RGB332: 0b00000011 = 0x03
        let pixel = CompactPixel::from_rgb(0, 0, 255);
        assert_eq!(pixel.color(), 0x03, "Blue should encode to 0x03");

        // White (255, 255, 255) -> RGB332: 0b11111111 = 0xFF
        let pixel = CompactPixel::from_rgb(255, 255, 255);
        assert_eq!(pixel.color(), 0xFF, "White should encode to 0xFF");

        // Black (0, 0, 0) -> RGB332: 0b00000000 = 0x00
        let pixel = CompactPixel::from_rgb(0, 0, 0);
        assert_eq!(pixel.color(), 0x00, "Black should encode to 0x00");
    }

    #[test]
    fn test_rgb332_decode() {
        set_rgb332_mode(true);

        // Decode red (0xE0) back to RGB
        let (r, g, b) = rgb332_to_rgb(0xE0);
        assert!(r > 200, "Red channel should be high, got {}", r);
        assert!(g < 50, "Green should be low, got {}", g);
        assert!(b < 50, "Blue should be low, got {}", b);

        // Decode green (0x1C)
        let (r, g, b) = rgb332_to_rgb(0x1C);
        assert!(r < 50, "Red should be low, got {}", r);
        assert!(g > 200, "Green should be high, got {}", g);
        assert!(b < 50, "Blue should be low, got {}", b);

        // Decode blue (0x03)
        let (r, g, b) = rgb332_to_rgb(0x03);
        assert!(r < 50, "Red should be low, got {}", r);
        assert!(g < 50, "Green should be low, got {}", g);
        assert!(b > 200, "Blue should be high, got {}", b);
    }

    #[test]
    fn test_intensity_scaling() {
        set_base_color(0xFF0000); // Red base (255, 0, 0)
        let pixel = CompactPixel::from_color(128); // Half intensity
        let (r, g, b) = pixel.to_rgb_intensity();
        // 128/255 * 255 ≈ 128 (half intensity red)
        assert!(r >= 126 && r <= 130, "Expected r around 128, got {}", r);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        // Test full intensity
        let pixel_full = CompactPixel::from_color(255);
        let (r2, _, _) = pixel_full.to_rgb_intensity();
        assert_eq!(r2, 255);

        // Test zero intensity
        let pixel_zero = CompactPixel::from_color(0);
        let (r3, _, _) = pixel_zero.to_rgb_intensity();
        assert_eq!(r3, 0);
    }

    #[test]
    fn test_canvas_fill() {
        let mut canvas = CompactCanvas::new(100, 100);
        canvas.fill_rect(10, 10, 50, 50, 200);

        // Check filled area
        assert_eq!(canvas.get_pixel(25, 25).color(), 200);
        // Check unfilled area
        assert_eq!(canvas.get_pixel(5, 5).color(), 0);
    }

    #[test]
    fn test_canvas_memory() {
        let canvas = CompactCanvas::new(800, 600);
        // 800 * 600 * 2 = 960,000 bytes (vs 1,920,000 for RGBA)
        assert_eq!(canvas.memory_size(), 960_000);
    }

    #[test]
    fn test_hex_parsing() {
        assert_eq!(hex_to_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("00FF00"), Some((0, 255, 0)));
        assert_eq!(hex_to_rgb("#0000FF"), Some((0, 0, 255)));
    }
}
