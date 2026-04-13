// src/runtime/family/asm/mod.rs
//! Adoption State Machine (ASM) — core engine for parent replacement,
//! reconciliation, and lineage stabilization.

pub mod verify;
pub mod select;
pub mod transfer;
pub mod update_child;
pub mod reconcile;

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{
    AdoptionEvent, AdoptionEventType, EventScope, FamilyId, MissingParent,
};
use crate::runtime::family::events::FamilyEventBus;
use crate::runtime::family::errors::LineageError;

/// All possible states of the ASM for a given adoption flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsmState {
    /// No adoption work is needed.
    Stable,
    /// A required parent is currently missing.
    ParentMissing,
    /// Verifying whether the missing parent is truly unavailable.
    Verifying,
    /// Choosing a replacement parent.
    SelectingReplacement,
    /// Copying semantic state to the replacement.
    TransferringSemantics,
    /// Updating the child to point at the new parent.
    UpdatingChild,
    /// Propagating lineage changes to dependents.
    Propagating,
    /// Reconciling final lineage state.
    Reconciling,
    /// Final stabilization before returning to steady state.
    Stabilizing,
}

impl std::fmt::Display for AsmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Run a full adoption flow for `child` when `missing` parent slot is absent.
///
/// Steps:
///   ParentMissing → Verifying → SelectingReplacement → TransferringSemantics
///   → UpdatingChild → Propagating → Stabilizing
pub fn run_adoption(
    child_id: FamilyId,
    missing: MissingParent,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    bus: &FamilyEventBus,
    now_ms: u64,
) -> Result<(), LineageError> {
    let _child_role = registry
        .get(&child_id)
        .map(|n| n.role)
        .ok_or_else(|| LineageError::ParentResolutionError { node_id: child_id, parent_id: 0 })?;

    // Emit AdoptionStarted
    let (pa, pb) = {
        let c = registry.get(&child_id).unwrap();
        (c.parent_a_id, c.parent_b_id)
    };
    bus.emit(AdoptionEvent {
        event_type: AdoptionEventType::AdoptionStarted,
        child_id, parent_a_id: pa, parent_b_id: pb,
        timestamp: now_ms, scope: EventScope::GlobalRootSpace,
    });

    // Verifying — determine the missing parent id
    let missing_parent_id = match missing {
        MissingParent::A => registry.get(&child_id).unwrap().parent_a_id,
        MissingParent::B => registry.get(&child_id).unwrap().parent_b_id,
    };

    let status = verify::asm_verify_parent_status(missing_parent_id, registry);
    if status == crate::runtime::family::types::ParentStatus::Visible {
        // False alarm — parent came back before we got here
        return Ok(());
    }

    // SelectingReplacement
    let new_parent_id = select::asm_select_replacement_parent(child_id, missing, registry, now_ms)
        .ok_or_else(|| LineageError::AdoptionTimeout {
            node_id: child_id,
            slot: format!("{:?}", missing),
        })?;

    // TransferringSemantics — copy old parent state into new parent
    // (old parent may be gone from registry; if so, snapshot from child lineage)
    if registry.contains_key(&missing_parent_id) {
        transfer::asm_transfer_semantics(missing_parent_id, new_parent_id, registry);
    }

    // UpdatingChild
    update_child::asm_update_child_parent_slot(child_id, new_parent_id, missing, registry, now_ms);

    // Emit ParentReplaced
    let (pa2, pb2) = {
        let c = registry.get(&child_id).unwrap();
        (c.parent_a_id, c.parent_b_id)
    };
    bus.emit(AdoptionEvent {
        event_type: AdoptionEventType::ParentReplaced,
        child_id, parent_a_id: pa2, parent_b_id: pb2,
        timestamp: now_ms, scope: EventScope::GlobalRootSpace,
    });

    // Stabilizing
    bus.emit(AdoptionEvent {
        event_type: AdoptionEventType::LineageStabilized,
        child_id, parent_a_id: pa2, parent_b_id: pb2,
        timestamp: now_ms, scope: EventScope::LocalUserspace,
    });

    Ok(())
}
