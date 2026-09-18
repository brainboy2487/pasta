use crate::interpreter::shell_os::vfs::{Node, Vfs};
use std::collections::HashMap;

/// Normalize a path string into a vector of components.
/// Handles ".", "..", absolute and relative paths.
/// ".." at the root is a no-op (cannot escape root).
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
                // P1-4: never pop past root
                if !parts.is_empty() {
                    parts.pop();
                }
            }
            other => parts.push(other.to_string()),
        }
    }

    parts
}

/// Walk `node` through `parts`, returning a mutable reference to the target node.
/// Safe alternative to raw-pointer traversal.
fn walk_mut<'a>(node: &'a mut Node, parts: &[String], original_path: &str) -> Result<&'a mut Node, String> {
    if parts.is_empty() {
        return Ok(node);
    }
    match node {
        Node::Dir(dir) => {
            let next = dir.children.get_mut(&parts[0])
                .ok_or_else(|| format!("Path not found: {}", original_path))?;
            walk_mut(next, &parts[1..], original_path)
        }
        Node::File(_) => Err(format!("Not a directory: {}", parts[0])),
    }
}

/// Walk `node` to the parent directory, returning its children map and the final component name.
fn walk_parent_mut<'a>(
    node: &'a mut Node,
    parts: &[String],
    original_path: &str,
) -> Result<(&'a mut HashMap<String, Node>, String), String> {
    if parts.is_empty() {
        return Err("Cannot operate on root".to_string());
    }
    let (parent_parts, last) = parts.split_at(parts.len() - 1);
    let name = last[0].clone();
    let parent = walk_mut(node, parent_parts, original_path)?;
    match parent {
        Node::Dir(dir) => Ok((&mut dir.children, name)),
        Node::File(_) => Err("Parent is not a directory".to_string()),
    }
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
        walk_mut(&mut self.root, &parts, path)
    }

    /// Resolve the parent directory of a path, returning:
    /// - mutable reference to parent directory node
    /// - final component name
    pub fn resolve_parent_mut(
        &mut self,
        path: &str,
    ) -> Result<(&mut HashMap<String, Node>, String), String> {
        let parts = normalize_path(&self.cwd, path);
        walk_parent_mut(&mut self.root, &parts, path)
    }
}
