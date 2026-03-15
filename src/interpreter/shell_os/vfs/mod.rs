/// Low-level filesystem helpers (read, write, persistence).
pub mod fs;
/// VFS node types: `File` and `Dir`.
pub mod node;
/// Path resolution and normalisation utilities.
pub mod path;

pub use node::Node;

/// The virtual filesystem instance.
pub struct Vfs {
    /// Root directory node of the filesystem tree.
    pub root: Node,
    /// Current working directory as a sequence of path components.
    pub cwd: Vec<String>,
    /// Path to the disk image backing this VFS instance.
    pub image_path: std::path::PathBuf,
    /// When true, commands operate on the real host filesystem instead of the VFS.
    pub local_mode: bool,
    /// Real host cwd used when local_mode is active.
    pub local_cwd: std::path::PathBuf,
}

impl Vfs {
    /// Create a new, empty VFS backed by the default image path.
    pub fn new_empty() -> Self {
        Vfs::new_empty_at(std::path::PathBuf::from("src/DiskImages/fs.img"))
    }

    /// Create a new, empty VFS backed by an explicit image path.
    pub fn new_empty_at(image_path: std::path::PathBuf) -> Self {
        Vfs {
            root: Node::new_dir(),
            cwd: Vec::new(),
            image_path,
            local_mode: false,
            local_cwd: std::path::PathBuf::from("/"),
        }
    }

    /// Return the prompt path string: real cwd in local mode, VFS path otherwise.
    pub fn prompt_path(&self) -> String {
        if self.local_mode {
            self.local_cwd.display().to_string()
        } else if self.cwd.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", self.cwd.join("/"))
        }
    }
}
