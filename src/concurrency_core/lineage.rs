// src/concurrency_core/lineage.rs
//! Simple parent-child lineage model for tasks.
//!
//! - Uses numeric `u64` task ids (TaskHandle.id).
//! - Minimal API for registering/unregistering nodes, querying parent/children,
//!   and basic adoption helpers.
//! - Designed to be a drop-in replacement for the previous string-keyed registry.

use crate::concurrency_core::task::TaskHandle;
use crate::concurrency_core::clock::LogicalClock;
use std::collections::HashMap;

/// A single node in the lineage graph.
#[derive(Debug, Clone)]
pub struct LineageNode {
    /// The task handle for this node.
    pub handle: TaskHandle,
    /// Optional parent handle.
    pub parent: Option<TaskHandle>,
    /// Direct children handles.
    pub children: Vec<TaskHandle>,
    /// Local logical clock snapshot for this node.
    pub clock: LogicalClock,
}

impl LineageNode {
    /// Create a new node from a handle. Parent is None and children empty.
    pub fn new(handle: TaskHandle) -> Self {
        Self {
            handle,
            parent: None,
            children: Vec::new(),
            clock: LogicalClock::zero(),
        }
    }

    /// Convenience: add a child handle (idempotent for identical handle ids).
    pub fn add_child(&mut self, child: TaskHandle) {
        if !self.children.iter().any(|c| c.id == child.id) {
            self.children.push(child);
        }
    }

    /// Remove a child by id. Returns true if removed.
    pub fn remove_child_by_id(&mut self, child_id: u64) -> bool {
        let orig = self.children.len();
        self.children.retain(|c| c.id != child_id);
        orig != self.children.len()
    }
}

/// Registry of lineage nodes keyed by numeric task id.
#[derive(Debug, Default)]
pub struct LineageRegistry {
    pub nodes: HashMap<u64, LineageNode>,
}

impl LineageRegistry {
    /// Create a new, empty registry.
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }

    /// Register a node. If a node with the same id already exists, it is replaced.
    /// Returns the previous node if any.
    pub fn register(&mut self, node: LineageNode) -> Option<LineageNode> {
        let id = node.handle.id;
        self.nodes.insert(id, node)
    }

    /// Unregister a node by id. Best-effort: also removes the node from its parent's children list.
    /// Returns the removed node if present.
    pub fn unregister(&mut self, id: u64) -> Option<LineageNode> {
        if let Some(node) = self.nodes.remove(&id) {
            // Remove from parent children list if parent exists
            if let Some(parent) = &node.parent {
                if let Some(pnode) = self.nodes.get_mut(&parent.id) {
                    pnode.remove_child_by_id(id);
                }
            }
            // Remove parent pointer from children (best-effort)
            for child in &node.children {
                if let Some(cnode) = self.nodes.get_mut(&child.id) {
                    cnode.parent = None;
                }
            }
            Some(node)
        } else {
            None
        }
    }

    /// Get an immutable reference to a node by handle.
    pub fn get(&self, handle: &TaskHandle) -> Option<&LineageNode> {
        self.nodes.get(&handle.id)
    }

    /// Get a mutable reference to a node by handle.
    pub fn get_mut(&mut self, handle: &TaskHandle) -> Option<&mut LineageNode> {
        self.nodes.get_mut(&handle.id)
    }

    /// Set parent for a node and update parent's children list.
    /// Returns Err if either node is missing.
    pub fn set_parent(&mut self, child_id: u64, parent_id: u64) -> Result<(), String> {
        if child_id == parent_id {
            return Err("cannot set node as its own parent".into());
        }
        let parent_exists = self.nodes.contains_key(&parent_id);
        let child_exists = self.nodes.contains_key(&child_id);
        if !parent_exists || !child_exists {
            return Err("child or parent not registered".into());
        }

        // update child's parent
        if let Some(child_node) = self.nodes.get_mut(&child_id) {
            child_node.parent = Some(TaskHandle::new(parent_id, child_node.handle.epoch));
        }

        // update parent's children
        if let Some(parent_node) = self.nodes.get_mut(&parent_id) {
            parent_node.add_child(TaskHandle::new(child_id, 0));
        }

        Ok(())
    }

    /// Remove parent relationship for a node. Best-effort.
    pub fn clear_parent(&mut self, child_id: u64) {
        if let Some(child_node) = self.nodes.get_mut(&child_id) {
            if let Some(parent) = child_node.parent.take() {
                if let Some(parent_node) = self.nodes.get_mut(&parent.id) {
                    parent_node.remove_child_by_id(child_id);
                }
            }
        }
    }

    /// Return the parent handle for a node, if any.
    pub fn parent_of(&self, id: u64) -> Option<TaskHandle> {
        self.nodes.get(&id).and_then(|n| n.parent.clone())
    }

    /// Return a vector of children handles for a node (cloned).
    pub fn children_of(&self, id: u64) -> Vec<TaskHandle> {
        self.nodes
            .get(&id)
            .map(|n| n.children.clone())
            .unwrap_or_default()
    }

    /// Update the stored logical clock for a node (best-effort).
    pub fn update_clock(&mut self, id: u64, clock: LogicalClock) {
        if let Some(node) = self.nodes.get_mut(&id) {
            node.clock = clock;
        }
    }

    /// Iterate over all nodes (immutable).
    pub fn iter(&self) -> impl Iterator<Item = (&u64, &LineageNode)> {
        self.nodes.iter()
    }

    /// Iterate over all nodes (mutable).
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&u64, &mut LineageNode)> {
        self.nodes.iter_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency_core::task::TaskHandle;

    #[test]
    fn register_and_query() {
        let mut reg = LineageRegistry::new();
        let h1 = TaskHandle::new(1, 0);
        let h2 = TaskHandle::new(2, 0);

        reg.register(LineageNode::new(h1));
        reg.register(LineageNode::new(h2));

        assert!(reg.get(&TaskHandle::new(1, 0)).is_some());
        assert!(reg.get(&TaskHandle::new(2, 0)).is_some());
    }

    #[test]
    fn parent_child_relationships() {
        let mut reg = LineageRegistry::new();
        let h1 = TaskHandle::new(1, 0);
        let h2 = TaskHandle::new(2, 0);

        reg.register(LineageNode::new(h1));
        reg.register(LineageNode::new(h2));

        reg.set_parent(2, 1).expect("set parent");
        assert_eq!(reg.parent_of(2).map(|p| p.id), Some(1));
        let children = reg.children_of(1);
        assert!(children.iter().any(|c| c.id == 2));

        reg.clear_parent(2);
        assert!(reg.parent_of(2).is_none());
    }

    #[test]
    fn unregister_cleans_up() {
        let mut reg = LineageRegistry::new();
        let h1 = TaskHandle::new(1, 0);
        let h2 = TaskHandle::new(2, 0);

        reg.register(LineageNode::new(h1));
        reg.register(LineageNode::new(h2));
        reg.set_parent(2, 1).unwrap();

        reg.unregister(1);
        // parent removed, child should have no parent
        assert!(reg.parent_of(2).is_none());
    }
}
