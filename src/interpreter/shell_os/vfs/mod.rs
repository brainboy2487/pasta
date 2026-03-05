pub mod fs;
pub mod node;
pub mod path;

pub use node::Node;
pub use fs::*;

/// The virtual filesystem instance.
pub struct Vfs {
    pub root: Node,
    pub cwd: Vec<String>,
}

impl Vfs {
    pub fn new_empty() -> Self {
        Vfs {
            root: Node::new_dir(),
            cwd: Vec::new(),
        }
    }
}
