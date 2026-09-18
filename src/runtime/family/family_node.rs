// src/runtime/family/family_node.rs
//! FamilyNode — the core runtime object for every non-primordial family member.

use crate::runtime::family::types::{
    DeltaEntry, FamilyId, LineageSignature, MinimalSnapshot, NodeRole, ObjGroup,
};

/// The core runtime object.  Every non-primordial entity in the family system
/// is a `FamilyNode` (whether it currently acts as a parent or a child).
#[derive(Debug, Clone)]
pub struct FamilyNode {
    /// Unique family id for this node.
    pub id:           FamilyId,

    // ── Parentage (always exactly two, except primordial grandparents) ──────
    /// Parent A family id.
    pub parent_a_id:  FamilyId,
    /// Parent B family id.
    pub parent_b_id:  FamilyId,

    // ── Role / group ─────────────────────────────────────────────────────────
    /// Current lineage role.
    pub role:         NodeRole,
    /// Object-group classification.
    pub group:        ObjGroup,

    // ── Evolution deltas ─────────────────────────────────────────────────────
    /// Mutation deltas accumulated on the node.
    pub mutations:    Vec<DeltaEntry>,
    /// Trait deltas accumulated on the node.
    pub traits:       Vec<DeltaEntry>,
    /// Capability deltas accumulated on the node.
    pub capabilities: Vec<DeltaEntry>,

    // ── Lineage integrity ─────────────────────────────────────────────────────
    /// Current lineage signature.
    pub lineage:      LineageSignature,

    // ── Parent-check config (must be explicit; inherits from child if unset) ─
    /// Optional interval between parent checks.
    pub check_interval_ms:  Option<u64>,
    /// Optional consecutive failure threshold before adoption.
    pub failure_threshold:  Option<u32>,

    // ── Timestamps ───────────────────────────────────────────────────────────
    /// Unix-ms of the most recent parent check.
    pub last_parent_check:   u64,   // unix-ms of most recent parent_check() run
    /// Unix-ms of the most recent adoption event.
    pub last_adoption_event: u64,   // unix-ms of most recent adoption event

    // ── Consecutive failure counters ─────────────────────────────────────────
    /// Consecutive failures observed for parent A.
    pub parent_a_failures: u32,
    /// Consecutive failures observed for parent B.
    pub parent_b_failures: u32,

    // ── Shadow parent tracking ───────────────────────────────────────────────
    /// If this node currently has a shadow parent for slot A.
    pub shadow_a_id:         Option<FamilyId>,
    /// Cycles elapsed since shadow parent A was created.
    pub shadow_a_cycles:     u32,   // cycles since shadow was created
    /// If this node currently has a shadow parent for slot B.
    pub shadow_b_id:         Option<FamilyId>,
    /// Cycles elapsed since shadow parent B was created.
    pub shadow_b_cycles:     u32,
}

impl FamilyNode {
    /// Construct a new family node with empty deltas and zeroed counters.
    pub fn new(
        id: FamilyId,
        parent_a_id: FamilyId,
        parent_b_id: FamilyId,
        role: NodeRole,
        group: ObjGroup,
        check_interval_ms: Option<u64>,
        failure_threshold: Option<u32>,
    ) -> Self {
        FamilyNode {
            id,
            parent_a_id,
            parent_b_id,
            role,
            group,
            mutations:    Vec::new(),
            traits:       Vec::new(),
            capabilities: Vec::new(),
            lineage:      LineageSignature { hash: 0, version: 0 },
            check_interval_ms,
            failure_threshold,
            last_parent_check:   0,
            last_adoption_event: 0,
            parent_a_failures:   0,
            parent_b_failures:   0,
            shadow_a_id:         None,
            shadow_a_cycles:     0,
            shadow_b_id:         None,
            shadow_b_cycles:     0,
        }
    }

    /// Recompute the lineage signature from current deltas.
    pub fn refresh_lineage(&mut self) {
        self.lineage = LineageSignature::compute(
            &self.mutations,
            &self.traits,
            &self.capabilities,
        );
    }

    /// Effective check interval — own value or inherited from child (caller
    /// must supply the child's interval when this is a parent node).
    pub fn effective_interval(&self, child_interval: Option<u64>) -> u64 {
        self.check_interval_ms.or(child_interval).unwrap_or(1000)
    }

    /// Effective failure threshold.
    pub fn effective_threshold(&self, child_threshold: Option<u32>) -> u32 {
        self.failure_threshold.or(child_threshold).unwrap_or(3)
    }

    /// Upsert a delta into a Vec<DeltaEntry> — latest wins per key.
    pub fn upsert_delta(entries: &mut Vec<DeltaEntry>, key: u32, value: i32, timestamp: u64) {
        if let Some(e) = entries.iter_mut().find(|e| e.key == key) {
            if timestamp >= e.timestamp {
                e.value     = value;
                e.timestamp = timestamp;
            }
        } else {
            entries.push(DeltaEntry { key, value, timestamp });
        }
    }

    /// Create a snapshot of current state.
    pub fn snapshot(&self) -> MinimalSnapshot {
        MinimalSnapshot {
            lineage:          self.lineage.clone(),
            mutation_count:   self.mutations.len() as u32,
            trait_count:      self.traits.len() as u32,
            capability_count: self.capabilities.len() as u32,
            timestamp:        self.last_parent_check,
            parent_a_id:      self.parent_a_id,
            parent_b_id:      self.parent_b_id,
        }
    }

    /// Restore delta counts and lineage from a snapshot (does not restore full deltas).
    pub fn restore_from_snapshot(&mut self, snap: &MinimalSnapshot) {
        self.lineage      = snap.lineage.clone();
        self.parent_a_id  = snap.parent_a_id;
        self.parent_b_id  = snap.parent_b_id;
        // Truncate deltas to snapshot counts (oldest entries kept)
        self.mutations.truncate(snap.mutation_count as usize);
        self.traits.truncate(snap.trait_count as usize);
        self.capabilities.truncate(snap.capability_count as usize);
    }

    /// Check if this node is eligible for GC.
    pub fn is_collectible(&self, parent_a_valid: bool, parent_b_valid: bool, retired: bool) -> bool {
        if retired { return true; }
        if !parent_a_valid || !parent_b_valid { return true; }
        // Invalid lineage signature (hash = 0 means never computed with real data)
        if self.lineage.hash == 0 && (!self.mutations.is_empty() || !self.traits.is_empty()) {
            return true;
        }
        false
    }
}
