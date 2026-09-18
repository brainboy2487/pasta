// src/runtime/family/asm/select.rs
//! Replacement-parent selection step of the ASM.
//!
//! Selection priority:
//!   1. Find any existing FamilyNode at the SAME NodeRole level as the missing parent.
//!   2. If none found at the same level, clone the missing parent (or child if parent
//!      is already gone from the registry).

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{FamilyId, MissingParent, NodeRole, next_family_id};

/// Select or create a replacement parent for `child`'s `missing` slot.
/// Returns the FamilyId of the replacement (which has been inserted into registry).
pub fn asm_select_replacement_parent(
    child_id: FamilyId,
    missing: MissingParent,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    now_ms: u64,
) -> Option<FamilyId> {
    let missing_parent_id = {
        let child = registry.get(&child_id)?;
        match missing {
            MissingParent::A => child.parent_a_id,
            MissingParent::B => child.parent_b_id,
        }
    };

    let target_role = NodeRole::Parent; // replacement is always at Parent level

    // 1. Search for an existing node at the same role level (not the missing parent itself,
    //    not the child, not primordials).
    let candidate = registry
        .iter()
        .find(|(&id, node)| {
            id != child_id
                && id != missing_parent_id
                && node.role == target_role
        })
        .map(|(&id, _)| id);

    if let Some(id) = candidate {
        return Some(id);
    }

    // 2. No same-level candidate — clone the missing parent (or use child as template).
    let source_id = if registry.contains_key(&missing_parent_id) {
        missing_parent_id
    } else {
        child_id
    };

    let new_id = next_family_id();
    let cloned = {
        let source = registry.get(&source_id)?.clone();
        FamilyNode {
            id:                   new_id,
            role:                 NodeRole::Parent,
            last_parent_check:    now_ms,
            last_adoption_event:  now_ms,
            parent_a_failures:    0,
            parent_b_failures:    0,
            shadow_a_id:          None,
            shadow_a_cycles:      0,
            shadow_b_id:          None,
            shadow_b_cycles:      0,
            ..source
        }
    };
    registry.insert(new_id, cloned);
    Some(new_id)
}
