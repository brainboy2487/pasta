use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::vfs::{Node, Vfs};

const FS_IMAGE: &str = "fs.img";

impl Vfs {
    /// Boot the virtual filesystem.
    /// If fs.img exists, load it. Otherwise create a new empty FS and save it.
    pub fn boot() -> Self {
        if Path::new(FS_IMAGE).exists() {
            match Self::load_from_disk() {
                Ok(vfs) => vfs,
                Err(_) => {
                    // Fallback: create a new FS if load fails.
                    let vfs = Self::new_empty();
                    let _ = vfs.save();
                    vfs
                }
            }
        } else {
            let vfs = Self::new_empty();
            let _ = vfs.save();
            vfs
        }
    }

    /// Load the filesystem image from disk.
    fn load_from_disk() -> Result<Self, String> {
        let mut file = File::open(FS_IMAGE)
            .map_err(|e| format!("Failed to open {}: {}", FS_IMAGE, e))?;

        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|e| format!("Failed reading {}: {}", FS_IMAGE, e))?;

        serde_json::from_str::<Node>(&buf)
            .map(|root| Vfs { root, cwd: Vec::new() })
            .map_err(|e| format!("Failed to parse FS image: {}", e))
    }

    /// Save the current filesystem state to disk.
    pub fn save(&self) -> Result<(), String> {
        let serialized = serde_json::to_string_pretty(&self.root)
            .map_err(|e| format!("Failed to serialize FS: {}", e))?;

        let mut file = File::create(FS_IMAGE)
            .map_err(|e| format!("Failed to create {}: {}", FS_IMAGE, e))?;

        file.write_all(serialized.as_bytes())
            .map_err(|e| format!("Failed to write FS image: {}", e))
    }
}
