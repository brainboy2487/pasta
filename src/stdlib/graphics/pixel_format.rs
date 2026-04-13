/// Standard RGB pixel format for all graphics operations
/// This unified format replaces the various RGB formats throughout PASTA
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// Construct an RGB color from channel components.
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Convert RGB to CompactCanvas format (16-bit packed)
    /// 5 bits red, 6 bits green, 5 bits blue
    pub fn to_compact(&self) -> u16 {
        let r = (self.r >> 3) as u16; // 8 bits -> 5 bits
        let g = (self.g >> 2) as u16; // 8 bits -> 6 bits
        let b = (self.b >> 3) as u16; // 8 bits -> 5 bits

        (r << 11) | (g << 5) | b
    }

    /// Convert from CompactCanvas format back to RGB
    pub fn from_compact(compact: u16) -> Self {
        let r = ((compact >> 11) & 0x1f) as u8;
        let g = ((compact >> 5) & 0x3f) as u8;
        let b = (compact & 0x1f) as u8;

        Self {
            r: r << 3, // 5 bits -> 8 bits
            g: g << 2, // 6 bits -> 8 bits
            b: b << 3, // 5 bits -> 8 bits
        }
    }

    /// Convert to 32-bit RGBA for X11 (with alpha = 255)
    pub fn to_rgba(&self) -> u32 {
        ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32) | 0xff000000
    }
}

/// Color constants for convenience
impl Rgb {
    /// Opaque black.
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
    /// Opaque white.
    pub const WHITE: Rgb = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    /// Opaque red.
    pub const RED: Rgb = Rgb { r: 255, g: 0, b: 0 };
    /// Opaque green.
    pub const GREEN: Rgb = Rgb { r: 0, g: 255, b: 0 };
    /// Opaque blue.
    pub const BLUE: Rgb = Rgb { r: 0, g: 0, b: 255 };
    /// Opaque yellow.
    pub const YELLOW: Rgb = Rgb {
        r: 255,
        g: 255,
        b: 0,
    };
    /// Opaque cyan.
    pub const CYAN: Rgb = Rgb {
        r: 0,
        g: 255,
        b: 255,
    };
    /// Opaque magenta.
    pub const MAGENTA: Rgb = Rgb {
        r: 255,
        g: 0,
        b: 255,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_conversion() {
        let original = Rgb::new(255, 128, 64);
        let compact = original.to_compact();
        let restored = Rgb::from_compact(compact);

        // Should be close due to bit reduction
        assert!((original.r as i16 - restored.r as i16).abs() <= 8);
        assert!((original.g as i16 - restored.g as i16).abs() <= 4);
        assert!((original.b as i16 - restored.b as i16).abs() <= 8);
    }
}
