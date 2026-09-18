// src/runtime/family/parent_check.rs
//! Parent check loop — heartbeat of the family system.
//!
//! Each FamilyNode runs parent_check() periodically on its own background thread
//! (driven by the pasta threading system).  In-process calls use the registry
//! directly; the distributed path sets ParentStatus before calling into the ASM.

use std::collections::HashMap;
use crate::runtime::family::asm;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{
    AdoptionEvent, AdoptionEventType, EventScope, FamilyId, MissingParent, ParentStatus,
};
use crate::runtime::family::asm::verify::asm_verify_parent_status;
use crate::runtime::family::events::FamilyEventBus;
use crate::runtime::family::errors::LineageError;

/// Run one parent-check cycle for `child_id`.
///
/// Returns the `DOES_PARENT_EXIST` boolean pair: (parent_a_ok, parent_b_ok).
/// After the call both parents will have been pushed the latest deltas OR adoption
/// will have been triggered.
pub fn parent_check(
    child_id: FamilyId,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    bus: &FamilyEventBus,
    now_ms: u64,
) -> Result<(bool, bool), LineageError> {
    // Read child state without a long borrow
    let (pa_id, pb_id) = {
        let child = registry.get(&child_id)
            .ok_or(LineageError::ParentResolutionError { node_id: child_id, parent_id: 0 })?;
        (child.parent_a_id, child.parent_b_id)
    };

    let status_a = asm_verify_parent_status(pa_id, registry);
    let status_b = asm_verify_parent_status(pb_id, registry);

    let a_ok = status_a == ParentStatus::Visible;
    let b_ok = status_b == ParentStatus::Visible;

    // Update failure counters and push updates or trigger adoption
    if a_ok {
        // Push updates to parent A
        push_updates(child_id, pa_id, registry);
        if let Some(child) = registry.get_mut(&child_id) {
            child.parent_a_failures = 0;
        }
    } else {
        let failures = {
            let child = registry.get_mut(&child_id).unwrap();
            child.parent_a_failures += 1;
            child.parent_a_failures
        };
        let threshold = registry.get(&child_id).unwrap().effective_threshold(None);

        if failures >= threshold {
            // Emit ParentMissing
            let (pa, pb) = { let c = registry.get(&child_id).unwrap(); (c.parent_a_id, c.parent_b_id) };
            bus.emit(AdoptionEvent {
                event_type: AdoptionEventType::ParentMissing,
                child_id, parent_a_id: pa, parent_b_id: pb,
                timestamp: now_ms, scope: EventScope::LocalUserspace,
            });
            // Trigger adoption
            asm::run_adoption(child_id, MissingParent::A, registry, bus, now_ms)?;
        }
    }

    if b_ok {
        push_updates(child_id, pb_id, registry);
        if let Some(child) = registry.get_mut(&child_id) {
            child.parent_b_failures = 0;
        }
    } else {
        let failures = {
            let child = registry.get_mut(&child_id).unwrap();
            child.parent_b_failures += 1;
            child.parent_b_failures
        };
        let threshold = registry.get(&child_id).unwrap().effective_threshold(None);

        if failures >= threshold {
            let (pa, pb) = { let c = registry.get(&child_id).unwrap(); (c.parent_a_id, c.parent_b_id) };
            bus.emit(AdoptionEvent {
                event_type: AdoptionEventType::ParentMissing,
                child_id, parent_a_id: pa, parent_b_id: pb,
                timestamp: now_ms, scope: EventScope::LocalUserspace,
            });
            asm::run_adoption(child_id, MissingParent::B, registry, bus, now_ms)?;
        }
    }

    // Tick shadow promotions
    asm::reconcile::tick_shadow_promotion(child_id, registry, bus, now_ms);

    // Update last_parent_check
    if let Some(child) = registry.get_mut(&child_id) {
        child.last_parent_check = now_ms;
    }

    Ok((a_ok, b_ok))
}

/// Push latest-wins deltas from child to parent (backward propagation).
/// Only the most recent DeltaEntry per key is sent.
fn push_updates(
    child_id: FamilyId,
    parent_id: FamilyId,
    registry: &mut HashMap<FamilyId, FamilyNode>,
) {
    // Collect child deltas
    let (mutations, traits, capabilities) = match registry.get(&child_id) {
        Some(c) => (c.mutations.clone(), c.traits.clone(), c.capabilities.clone()),
        None => return,
    };

    if let Some(parent) = registry.get_mut(&parent_id) {
        for d in mutations   { FamilyNode::upsert_delta(&mut parent.mutations,    d.key, d.value, d.timestamp); }
        for d in traits      { FamilyNode::upsert_delta(&mut parent.traits,       d.key, d.value, d.timestamp); }
        for d in capabilities { FamilyNode::upsert_delta(&mut parent.capabilities, d.key, d.value, d.timestamp); }
        parent.refresh_lineage();
    }
}

/// `DOES_PARENT_EXIST` — simple boolean check for pasta user code.
/// Returns true only if both parents are currently visible.
pub fn does_parent_exist(child_id: FamilyId, registry: &HashMap<FamilyId, FamilyNode>) -> bool {
    match registry.get(&child_id) {
        None => false,
        Some(child) => {
            registry.contains_key(&child.parent_a_id)
                && registry.contains_key(&child.parent_b_id)
        }
    }
}
