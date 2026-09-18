// src/runtime/family/asm/transfer.rs
//! Semantics transfer step of the ASM.
//!
//! Copies old parent's mutations, traits, capabilities, and lineage into the
//! new (adoptive) parent.  After transfer the new parent co-evolves with the
//! child going forward (Option B).

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::FamilyId;

/// Copy old parent's semantic state into new parent.
pub fn asm_transfer_semantics(
    old_parent_id: FamilyId,
    new_parent_id: FamilyId,
    registry: &mut HashMap<FamilyId, FamilyNode>,
) {
    // Extract old state first to avoid borrow conflicts.
    let (mutations, traits, capabilities, lineage) = {
        let old = match registry.get(&old_parent_id) {
            Some(n) => n,
            None => return,
        };
        (old.mutations.clone(), old.traits.clone(), old.capabilities.clone(), old.lineage.clone())
    };

    if let Some(new_parent) = registry.get_mut(&new_parent_id) {
        // Merge — latest wins per key for each delta vector.
        for d in mutations {
            FamilyNode::upsert_delta(&mut new_parent.mutations, d.key, d.value, d.timestamp);
        }
        for d in traits {
            FamilyNode::upsert_delta(&mut new_parent.traits, d.key, d.value, d.timestamp);
        }
        for d in capabilities {
            FamilyNode::upsert_delta(&mut new_parent.capabilities, d.key, d.value, d.timestamp);
        }
        // Accept old lineage signature as baseline; version bumped by one to mark adoption.
        new_parent.lineage = lineage;
        new_parent.lineage.version += 1;
        new_parent.refresh_lineage();
    }
}
