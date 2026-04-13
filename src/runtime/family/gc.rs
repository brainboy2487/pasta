// src/runtime/family/gc.rs
//! GC hooks for the Pasta Family Object System.

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::FamilyId;

/// Check if a node is eligible for garbage collection.
///
/// Rules:
/// - Cannot anchor to two valid parents → collectible.
/// - Lineage signature invalid → collectible.
/// - Explicitly retired → collectible.
pub fn gc_is_collectible(
    node: &FamilyNode,
    registry: &HashMap<FamilyId, FamilyNode>,
    retired: bool,
) -> bool {
    if retired { return true; }
    let parent_a_valid = registry.contains_key(&node.parent_a_id);
    let parent_b_valid = registry.contains_key(&node.parent_b_id);
    node.is_collectible(parent_a_valid, parent_b_valid, false)
}

/// Remove a node from the registry (GC collect).
/// Caller must have already verified the node is collectible.
pub fn gc_collect(node_id: FamilyId, registry: &mut HashMap<FamilyId, FamilyNode>) {
    registry.remove(&node_id);
}
