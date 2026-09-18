// src/runtime/family/registry.rs
//! FamilyRegistry — central in-memory store for all FamilyNodes.
//!
//! Wraps a HashMap<FamilyId, FamilyNode> plus the two primordials and the
//! event bus.  This is what the interpreter holds.

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::grandparent::Primordials;
use crate::runtime::family::types::{FamilyId, ObjGroup, NodeRole, next_family_id};
use crate::runtime::family::events::{FamilyEventBus, UnsafePermission};
use crate::runtime::family::errors::LineageError;
use crate::runtime::family::parent_check::{parent_check, does_parent_exist};
use crate::runtime::family::gc::{gc_is_collectible, gc_collect};

/// Central registry for family nodes, primordials, and lineage events.
pub struct FamilyRegistry {
    /// Active family nodes keyed by family id.
    pub nodes:      HashMap<FamilyId, FamilyNode>,
    /// Primordial lineage roots.
    pub primordials: Primordials,
    /// Event bus for lineage lifecycle events.
    pub bus:        FamilyEventBus,
    retired:        std::collections::HashSet<FamilyId>,
}

impl FamilyRegistry {
    /// Create an empty family registry with the given unsafe permission level.
    pub fn new(permission: UnsafePermission) -> Self {
        FamilyRegistry {
            nodes:       HashMap::new(),
            primordials: Primordials::initialize(None, None),
            bus:         FamilyEventBus::new(permission),
            retired:     std::collections::HashSet::new(),
        }
    }

    /// Create a new FamilyNode and insert it into the registry.
    /// Validates cross-type parenting rules.
    pub fn create_node(
        &mut self,
        parent_a_id: FamilyId,
        parent_b_id: FamilyId,
        role: NodeRole,
        group: ObjGroup,
        check_interval_ms: Option<u64>,
        failure_threshold: Option<u32>,
    ) -> Result<FamilyId, LineageError> {
        // Validate group type compatibility
        if let Some(pa) = self.nodes.get(&parent_a_id) {
            if !group.can_have_parent(&pa.group) {
                return Err(LineageError::GroupTypeMismatch {
                    child_group:  format!("{:?}", group),
                    parent_group: format!("{:?}", pa.group),
                });
            }
        }
        if let Some(pb) = self.nodes.get(&parent_b_id) {
            if !group.can_have_parent(&pb.group) {
                return Err(LineageError::GroupTypeMismatch {
                    child_group:  format!("{:?}", group),
                    parent_group: format!("{:?}", pb.group),
                });
            }
        }

        let id = next_family_id();
        let node = FamilyNode::new(
            id, parent_a_id, parent_b_id, role, group,
            check_interval_ms, failure_threshold,
        );
        self.nodes.insert(id, node);
        Ok(id)
    }

    /// Run a parent-check cycle for a node.
    pub fn check(&mut self, child_id: FamilyId, now_ms: u64) -> Result<(bool, bool), LineageError> {
        parent_check(child_id, &mut self.nodes, &self.bus, now_ms)
    }

    /// DOES_PARENT_EXIST for pasta user code.
    pub fn does_parent_exist(&self, child_id: FamilyId) -> bool {
        does_parent_exist(child_id, &self.nodes)
    }

    /// Mark a node as retired (eligible for GC).
    pub fn retire(&mut self, id: FamilyId) {
        self.retired.insert(id);
    }

    /// Run GC pass — remove all collectible nodes.
    pub fn gc_pass(&mut self) {
        let collectible: Vec<FamilyId> = self.nodes
            .keys()
            .copied()
            .filter(|id| gc_is_collectible(
                self.nodes.get(id).unwrap(),
                &self.nodes,
                self.retired.contains(id),
            ))
            .collect();
        for id in collectible {
            gc_collect(id, &mut self.nodes);
            self.retired.remove(&id);
        }
    }

    /// Return an immutable view of a node by family id.
    pub fn get(&self, id: FamilyId) -> Option<&FamilyNode> {
        self.nodes.get(&id)
    }

    /// Return a mutable view of a node by family id.
    pub fn get_mut(&mut self, id: FamilyId) -> Option<&mut FamilyNode> {
        self.nodes.get_mut(&id)
    }
}
