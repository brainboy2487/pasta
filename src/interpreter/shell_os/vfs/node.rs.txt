use serde::{Serialize, Deserialize};
use std::collections::HashMap;

/// A node in the virtual filesystem.
/// Either a directory with children or a file with raw bytes.
#[derive(Serialize, Deserialize, Clone)]
pub enum Node {
    File(FileData),
    Dir(DirData),
}

/// File contents stored as raw bytes.
#[derive(Serialize, Deserialize, Clone)]
pub struct FileData {
    pub data: Vec<u8>,
}

/// Directory contents stored as a map of name -> Node.
#[derive(Serialize, Deserialize, Clone)]
pub struct DirData {
    pub children: HashMap<String, Node>,
}

impl Node {
    /// Create an empty directory node.
    pub fn new_dir() -> Self {
        Node::Dir(DirData {
            children: HashMap::new(),
        })
    }

    /// Create a file node with given bytes.
    pub fn new_file(data: Vec<u8>) -> Self {
        Node::File(FileData { data })
    }

    /// Returns true if this node is a directory.
    pub fn is_dir(&self) -> bool {
        matches!(self, Node::Dir(_))
    }

    /// Returns true if this node is a file.
    pub fn is_file(&self) -> bool {
        matches!(self, Node::File(_))
    }

    /// Get immutable directory children, if this is a directory.
    pub fn children(&self) -> Option<&HashMap<String, Node>> {
        match self {
            Node::Dir(dir) => Some(&dir.children),
            _ => None,
        }
    }

    /// Get mutable directory children, if this is a directory.
    pub fn children_mut(&mut self) -> Option<&mut HashMap<String, Node>> {
        match self {
            Node::Dir(dir) => Some(&mut dir.children),
            _ => None,
        }
    }

    /// Get immutable file data, if this is a file.
    pub fn file_data(&self) -> Option<&[u8]> {
        match self {
            Node::File(f) => Some(&f.data),
            _ => None,
        }
    }

    /// Get mutable file data, if this is a file.
    pub fn file_data_mut(&mut self) -> Option<&mut Vec<u8>> {
        match self {
            Node::File(f) => Some(&mut f.data),
            _ => None,
        }
    }
}
