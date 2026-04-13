// src/runtime/family/asm/update_child.rs
//! Child parent-slot update step of the ASM.

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{FamilyId, MissingParent};

/// Update the child's parent_a_id or parent_b_id to the new parent.
pub fn asm_update_child_parent_slot(
    child_id: FamilyId,
    new_parent_id: FamilyId,
    missing: MissingParent,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    now_ms: u64,
) {
    if let Some(child) = registry.get_mut(&child_id) {
        match missing {
            MissingParent::A => child.parent_a_id = new_parent_id,
            MissingParent::B => child.parent_b_id = new_parent_id,
        }
        child.last_adoption_event = now_ms;
        // Reset failure counter for the replaced slot.
        match missing {
            MissingParent::A => child.parent_a_failures = 0,
            MissingParent::B => child.parent_b_failures = 0,
        }
    }
}
