// src/stdlib/imports/mod.rs
//! Minimal import handler helpers.
//! This file provides a tiny helper that recognizes the simple FROM: ... END
//! block form and calls module-specific import shims (e.g., graphics).

use crate::interpreter::Runtime;

/// Handle a FROM: ... END block (raw block text).
/// This is intentionally small: it extracts module names and calls the
/// corresponding import shim. It does not implement a full import parser.
pub fn handle_from_block(block: &str, rt: &mut Runtime) {
    // Normalize whitespace and lowercase for matching.
    let text = block.to_lowercase();

    // Find module names between FROM: and the first nested END (simple heuristic).
    // Accept lines like: <graphics> or graphics
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('<') && line.ends_with('>') {
            let name = &line[1..line.len()-1];
            match name {
                "graphics" => {
                    // Call the graphics import shim
                    crate::stdlib::imports::graphics_import::import_graphics(rt);
                }
                _ => {
                    // Unknown module: ignore for now or log.
                }
            }
        } else if !line.is_empty() && !line.starts_with("from:") && !line.starts_with("use:") && !line.starts_with("end") {
            // also accept bare module names
            let name = line.split_whitespace().next().unwrap_or("");
            match name {
                "graphics" => {
                    crate::stdlib::imports::graphics_import::import_graphics(rt);
                }
                _ => {}
            }
        }
    }
}
