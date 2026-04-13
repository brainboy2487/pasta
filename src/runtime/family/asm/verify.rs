// src/runtime/family/asm/verify.rs
//! Parent status verification step of the ASM.

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{FamilyId, ParentStatus};
use crate::runtime::family::grandparent::Primordials;

/// Determine the current status of a parent node.
///
/// - If the parent is in the registry → Visible.
/// - If the parent is a primordial (IDs 1/2) → always Visible (immutable).
/// - Otherwise → Dead (not recoverable by the runtime at this call site).
///
/// In a distributed context the threading layer may set the status to
/// Unreachable or Missing before calling the ASM; the ASM itself only
/// distinguishes Visible vs not-Visible for the adoption decision.
pub fn asm_verify_parent_status(
    parent_id: FamilyId,
    registry: &HashMap<FamilyId, FamilyNode>,
) -> ParentStatus {
    if Primordials::is_primordial(parent_id) {
        return ParentStatus::Visible;
    }
    if registry.contains_key(&parent_id) {
        ParentStatus::Visible
    } else {
        ParentStatus::Dead
    }
}
