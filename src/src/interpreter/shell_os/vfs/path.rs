use crate::vfs::{Node, Vfs};
use std::collections::HashMap;

/// Normalize a path string into a vector of components.
/// Handles ".", "..", absolute and relative paths.
pub fn normalize_path(cwd: &[String], input: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();

    if input.starts_with('/') {
        // Absolute path: start from root
    } else {
        // Relative path: start from cwd
        parts.extend(cwd.iter().cloned());
    }

    for part in input.split('/') {
        match part {
            "" | "." => continue,
            ".." => {
                parts.pop();
            }
            other => parts.push(other.to_string()),
        }
    }

    parts
}

impl Vfs {
    /// Resolve a path to an immutable reference to a Node.
    pub fn resolve(&self, path: &str) -> Result<&Node, String> {
        let parts = normalize_path(&self.cwd, path);
        let mut current = &self.root;

        for p in parts {
            match current {
                Node::Dir(dir) => {
                    current = dir.children.get(&p)
                        .ok_or_else(|| format!("Path not found: {}", path))?;
                }
                Node::File(_) => {
                    return Err(format!("Not a directory: {}", p));
                }
            }
        }

        Ok(current)
    }

    /// Resolve a path to a mutable reference to a Node.
    pub fn resolve_mut(&mut self, path: &str) -> Result<&mut Node, String> {
        let parts = normalize_path(&self.cwd, path);
        let mut current: *mut Node = &mut self.root;

        for p in parts {
            unsafe {
                match &mut *current {
                    Node::Dir(dir) => {
                        let next = dir.children.get_mut(&p)
                            .ok_or_else(|| format!("Path not found: {}", path))?;
                        current = next;
                    }
                    Node::File(_) => {
                        return Err(format!("Not a directory: {}", p));
                    }
                }
            }
        }

        unsafe { Ok(&mut *current) }
    }

    /// Resolve the parent directory of a path, returning:
    /// - mutable reference to parent directory node
    /// - final component name
    pub fn resolve_parent_mut(
        &mut self,
        path: &str,
    ) -> Result<(&mut HashMap<String, Node>, String), String> {
        let parts = normalize_path(&self.cwd, path);

        if parts.is_empty() {
            return Err("Cannot operate on root".to_string());
        }

        let (parent_parts, name) = parts.split_at(parts.len() - 1);
        let name = name[0].clone();

        // Walk to parent directory
        let mut current: *mut Node = &mut self.root;

        for p in parent_parts {
            unsafe {
                match &mut *current {
                    Node::Dir(dir) => {
                        let next = dir.children.get_mut(p)
                            .ok_or_else(|| format!("Path not found: {}", path))?;
                        current = next;
                    }
                    Node::File(_) => {
                        return Err(format!("Not a directory: {}", p));
                    }
                }
            }
        }

        unsafe {
            match &mut *current {
                Node::Dir(dir) => Ok((&mut dir.children, name)),
                Node::File(_) => Err("Parent is not a directory".to_string()),
            }
        }
    }
}
