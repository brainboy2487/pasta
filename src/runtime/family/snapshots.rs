// src/runtime/family/snapshots.rs
//! Snapshot and recovery API for FamilyNodes.

use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::MinimalSnapshot;

/// Create a minimal snapshot of a FamilyNode's current state.
pub fn create_snapshot(node: &FamilyNode) -> MinimalSnapshot {
    node.snapshot()
}

/// Restore a FamilyNode from a snapshot.
pub fn restore_from_snapshot(node: &mut FamilyNode, snapshot: &MinimalSnapshot) {
    node.restore_from_snapshot(snapshot);
}
