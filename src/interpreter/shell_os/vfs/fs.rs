use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::interpreter::shell_os::vfs::{Node, Vfs};

/// Default VFS image path (relative to the project root).
pub const DEFAULT_FS_IMAGE: &str = "src/DiskImages/fs.img";

impl Vfs {
    /// Boot a VFS from the given image path.
    /// If the image exists and is valid JSON, load it.
    /// Otherwise create a fresh empty VFS, persist it, and return it.
    pub fn boot(image_path: PathBuf) -> Self {
        if image_path.exists() {
            match Self::load_from_path(&image_path) {
                Ok(mut vfs) => {
                    vfs.image_path = image_path;
                    return vfs;
                }
                Err(e) => {
                    eprintln!("[vfs] load failed ({}), creating fresh FS", e);
                }
            }
        } else {
            if let Some(parent) = image_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        let vfs = Vfs::new_empty_at(image_path);
        let _ = vfs.save();
        vfs
    }

    /// Boot from the default image path.
    pub fn boot_default() -> Self {
        Self::boot(PathBuf::from(DEFAULT_FS_IMAGE))
    }

    /// Load a VFS from an explicit path.
    fn load_from_path(path: &Path) -> Result<Self, String> {
        let mut file = File::open(path)
            .map_err(|e| format!("open {:?}: {}", path, e))?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)
            .map_err(|e| format!("read {:?}: {}", path, e))?;
        serde_json::from_str::<Node>(&buf)
            .map(|root| Vfs {
                root,
                cwd: Vec::new(),
                image_path: path.to_path_buf(),
                local_mode: false,
                local_cwd: std::path::PathBuf::from("/"),
            })
            .map_err(|e| format!("parse {:?}: {}", path, e))
    }

    /// Save the current filesystem state to self.image_path.
    pub fn save(&self) -> Result<(), String> {
        if let Some(parent) = self.image_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("mkdir {:?}: {}", parent, e))?;
        }
        let serialized = serde_json::to_string_pretty(&self.root)
            .map_err(|e| format!("serialize: {}", e))?;
        let mut file = File::create(&self.image_path)
            .map_err(|e| format!("create {:?}: {}", self.image_path, e))?;
        file.write_all(serialized.as_bytes())
            .map_err(|e| format!("write {:?}: {}", self.image_path, e))
    }

    /// Swap to a new image path: save current state, then load/create the new image.
    pub fn mount(&mut self, new_path: PathBuf) -> Result<(), String> {
        if !self.local_mode {
            self.save()?;
        }
        *self = Vfs::boot(new_path);
        Ok(())
    }

    /// Switch to local (host) filesystem mode.
    pub fn mount_local(&mut self) {
        // Save VFS image before switching away (best-effort).
        if !self.local_mode {
            let _ = self.save();
        }
        self.local_mode = true;
        self.local_cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/"));
    }
}
