// src/runtime/family/asm/reconcile.rs
//! Reconciliation — handles a returning parent after adoption (Option C).
//!
//! Tie-break order:
//!   1. More recent timestamp wins.
//!   2. Equal timestamps → higher mutation count wins.
//!   3. Equal mutation counts → least-modified hash (lowest XOR popcount) wins.

use std::collections::HashMap;
use crate::runtime::family::family_node::FamilyNode;
use crate::runtime::family::types::{
    AdoptionEvent, AdoptionEventType, EventScope, FamilyId, MissingParent, ReconciliationOutcome,
};
use crate::runtime::family::events::FamilyEventBus;

pub fn asm_reconcile_returning_parent(
    child_id:           FamilyId,
    adoptive_parent_id: FamilyId,
    returning_parent_id: FamilyId,
    missing: MissingParent,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    bus: &FamilyEventBus,
    now_ms: u64,
) -> ReconciliationOutcome {
    // Collect comparison data without holding a borrow.
    let (ap_ts, ap_mc, ap_hash) = match registry.get(&adoptive_parent_id) {
        Some(n) => (n.last_parent_check, n.mutations.len() as u64, n.lineage.hash),
        None    => (0, 0, 0),
    };
    let (rp_ts, rp_mc, rp_hash) = match registry.get(&returning_parent_id) {
        Some(n) => (n.last_parent_check, n.mutations.len() as u64, n.lineage.hash),
        None    => (0, 0, 0),
    };

    // Determine winner
    let adoptive_wins = if ap_ts != rp_ts {
        ap_ts > rp_ts
    } else if ap_mc != rp_mc {
        ap_mc > rp_mc
    } else {
        // Least-modified = fewest bits changed from a baseline of 0
        // (the hash that has fewer set bits after XOR with itself = the "cleaner" one).
        // We compare how many bits each hash has set — the smaller popcount wins.
        ap_hash.count_ones() <= rp_hash.count_ones()
    };

    let outcome = if adoptive_wins {
        // Demote returning parent to shadow
        let shadow_id = returning_parent_id;
        match missing {
            MissingParent::A => {
                if let Some(child) = registry.get_mut(&child_id) {
                    child.shadow_a_id     = Some(shadow_id);
                    child.shadow_a_cycles = 0;
                }
            }
            MissingParent::B => {
                if let Some(child) = registry.get_mut(&child_id) {
                    child.shadow_b_id     = Some(shadow_id);
                    child.shadow_b_cycles = 0;
                }
            }
        }
        ReconciliationOutcome::ShadowParentCreated(shadow_id)
    } else {
        // Returning parent wins — swap it back into the active slot
        if let Some(child) = registry.get_mut(&child_id) {
            match missing {
                MissingParent::A => {
                    child.shadow_a_id     = Some(adoptive_parent_id);
                    child.shadow_a_cycles = 0;
                    child.parent_a_id     = returning_parent_id;
                }
                MissingParent::B => {
                    child.shadow_b_id     = Some(adoptive_parent_id);
                    child.shadow_b_cycles = 0;
                    child.parent_b_id     = returning_parent_id;
                }
            }
            child.last_adoption_event = now_ms;
        }
        ReconciliationOutcome::ReturningParentWins
    };

    // Emit Reconciliation event
    let (pa, pb) = {
        let c = registry.get(&child_id).unwrap();
        (c.parent_a_id, c.parent_b_id)
    };
    bus.emit(AdoptionEvent {
        event_type: AdoptionEventType::Reconciliation,
        child_id, parent_a_id: pa, parent_b_id: pb,
        timestamp: now_ms, scope: EventScope::GlobalRootSpace,
    });

    outcome
}

/// Tick shadow parent promotion. Call once per parent-check cycle for every child.
/// Shadow promotes back to active parent after 3× failure_threshold missed cycles.
pub fn tick_shadow_promotion(
    child_id: FamilyId,
    registry: &mut HashMap<FamilyId, FamilyNode>,
    bus: &FamilyEventBus,
    now_ms: u64,
) {
    let (threshold, shadow_a, shadow_a_cycles, shadow_b, shadow_b_cycles,
         _failure_threshold, pa, pb) = {
        let child = match registry.get(&child_id) { Some(c) => c, None => return };
        let ft = child.effective_threshold(None);
        (ft * 3,
         child.shadow_a_id, child.shadow_a_cycles,
         child.shadow_b_id, child.shadow_b_cycles,
         ft,
         child.parent_a_id, child.parent_b_id)
    };

    // Shadow A
    if let Some(shadow_id) = shadow_a {
        let new_cycles = shadow_a_cycles + 1;
        if new_cycles >= threshold {
            // Promote shadow back to slot A
            if let Some(child) = registry.get_mut(&child_id) {
                child.parent_a_id     = shadow_id;
                child.shadow_a_id     = None;
                child.shadow_a_cycles = 0;
                child.last_adoption_event = now_ms;
            }
            bus.emit(AdoptionEvent {
                event_type: AdoptionEventType::ParentReturned,
                child_id, parent_a_id: shadow_id, parent_b_id: pb,
                timestamp: now_ms, scope: EventScope::GlobalRootSpace,
            });
        } else if let Some(child) = registry.get_mut(&child_id) {
            child.shadow_a_cycles = new_cycles;
        }
    }

    // Shadow B
    if let Some(shadow_id) = shadow_b {
        let new_cycles = shadow_b_cycles + 1;
        if new_cycles >= threshold {
            if let Some(child) = registry.get_mut(&child_id) {
                child.parent_b_id     = shadow_id;
                child.shadow_b_id     = None;
                child.shadow_b_cycles = 0;
                child.last_adoption_event = now_ms;
            }
            bus.emit(AdoptionEvent {
                event_type: AdoptionEventType::ParentReturned,
                child_id, parent_a_id: pa, parent_b_id: shadow_id,
                timestamp: now_ms, scope: EventScope::GlobalRootSpace,
            });
        } else if let Some(child) = registry.get_mut(&child_id) {
            child.shadow_b_cycles = new_cycles;
        }
    }
}
