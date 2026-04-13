// src/runtime/family/types.rs
//! Core data structures for the Pasta Family Object System.

use std::sync::atomic::{AtomicU64, Ordering};

// ── FamilyId ──────────────────────────────────────────────────────────────────

/// Stable identifier for a node in the family runtime.
pub type FamilyId = u64;

static NEXT_ID: AtomicU64 = AtomicU64::new(3); // 1 and 2 reserved for primordials

/// Allocate the next runtime family id.
pub fn next_family_id() -> FamilyId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// ── NodeRole ──────────────────────────────────────────────────────────────────

/// Category/role label on every FamilyNode.
/// Used by the ASM to restrict replacement-parent searches to the same level,
/// and for debugging/logging clarity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum NodeRole {
    /// Root-level lineage anchor.
    Grandparent = 0,
    /// Intermediate parent node.
    Parent      = 1,
    /// Leaf or child node.
    Child       = 2,
}

// ── OBJ group type ────────────────────────────────────────────────────────────

/// Which OBJ.FAM group a node belongs to.
/// Cross-type parenting: LST/DICT/TNSR/NRML parents must match child type.
/// OBJ.CSM can have parents of any group type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObjGroup {
    /// List-backed family object.
    Lst,
    /// Dictionary-backed family object.
    Dict,
    /// Tensor-backed family object.
    Tnsr,
    /// Normal/general object group.
    Nrml,
    /// Custom group: declares a primary group at creation, extends via USE statements.
    Csm {
        /// Primary underlying object group.
        primary: Box<ObjGroup>,
        /// Additional group extensions enabled for the object.
        extensions: Vec<ObjGroup>,
    },
}

impl ObjGroup {
    /// Returns true if `parent_group` is a valid parent for a child of `self`.
    pub fn can_have_parent(&self, parent_group: &ObjGroup) -> bool {
        match self {
            ObjGroup::Csm { .. } => true, // CSM cross-types freely
            other => {
                // For CSM parents, check their primary
                match parent_group {
                    ObjGroup::Csm { primary, .. } => other == primary.as_ref(),
                    p => other == p,
                }
            }
        }
    }
}

// ── DeltaEntry ────────────────────────────────────────────────────────────────

/// Unified delta type replacing MutationDelta, TraitDelta, and CapabilityDelta.
/// key  = opaque u32 handle; meaning is entirely user-defined.
/// value = i32; for capabilities: 0 = disabled, 1 = enabled.
#[derive(Debug, Clone)]
pub struct DeltaEntry {
    /// User-defined delta key.
    pub key:       u32,
    /// Delta value or enabled/disabled state.
    pub value:     i32,
    /// Timestamp associated with the delta.
    pub timestamp: u64,
}

// ── LineageSignature ──────────────────────────────────────────────────────────

/// Integrity marker computed from all current delta keys and values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineageSignature {
    /// Hash of the current lineage-relevant state.
    pub hash:    u64,
    /// Signature schema/version.
    pub version: u32,
}

impl LineageSignature {
    /// Compute a deterministic signature from mutations, traits, and capabilities.
    pub fn compute(
        mutations:    &[DeltaEntry],
        traits:       &[DeltaEntry],
        capabilities: &[DeltaEntry],
    ) -> Self {
        // Simple deterministic hash: FNV-1a over all (key, value) pairs in order.
        let mut h: u64 = 0xcbf29ce484222325;
        for d in mutations.iter().chain(traits).chain(capabilities) {
            h ^= d.key as u64;
            h = h.wrapping_mul(0x100000001b3);
            h ^= d.value as i64 as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        LineageSignature { hash: h, version: 0 }
    }

    /// Number of bits that differ between two signatures (used for tie-breaking:
    /// least-modified = fewer bits changed = lower popcount of XOR).
    pub fn change_distance(&self, other: &LineageSignature) -> u32 {
        (self.hash ^ other.hash).count_ones()
    }
}

// ── MinimalSnapshot ───────────────────────────────────────────────────────────

/// Snapshot used for recovery, reconciliation, and adoption logging.
#[derive(Debug, Clone)]
pub struct MinimalSnapshot {
    /// Current lineage signature.
    pub lineage:          LineageSignature,
    /// Count of mutation deltas.
    pub mutation_count:   u32,
    /// Count of trait deltas.
    pub trait_count:      u32,
    /// Count of capability deltas.
    pub capability_count: u32,
    /// Snapshot timestamp.
    pub timestamp:        u64,
    /// Parent A id at snapshot time.
    pub parent_a_id:      FamilyId,
    /// Parent B id at snapshot time.
    pub parent_b_id:      FamilyId,
}

// ── ParentStatus ──────────────────────────────────────────────────────────────

/// Visibility status of a parent during lineage checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParentStatus {
    /// Parent is present and reachable.
    Visible,
    /// Parent is absent from the expected location.
    Missing,
    /// Parent exists conceptually but cannot be reached.
    Unreachable,
    /// Parent is known dead/retired.
    Dead,
}

// ── MissingParent slot ────────────────────────────────────────────────────────

/// Which parent slot is currently missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingParent {
    /// Parent A slot.
    A,
    /// Parent B slot.
    B,
}

// ── ReconciliationOutcome ─────────────────────────────────────────────────────

/// Result of reconciling returning and adoptive lineage state.
#[derive(Debug, Clone)]
pub enum ReconciliationOutcome {
    /// The adoptive parent remains authoritative.
    AdoptiveParentWins,
    /// The returning original parent wins reconciliation.
    ReturningParentWins,
    /// A new shadow parent was created with the given id.
    ShadowParentCreated(FamilyId),
}

// ── EventScope ────────────────────────────────────────────────────────────────

/// Visibility scope for family-system events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventScope {
    /// Event visible only in local userspace.
    LocalUserspace,
    /// Event visible in the global root space.
    GlobalRootSpace,
}

// ── AdoptionEventType ─────────────────────────────────────────────────────────

/// Event kinds emitted during adoption and reconciliation flows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdoptionEventType {
    /// A required parent went missing.
    ParentMissing,
    /// Adoption flow started.
    AdoptionStarted,
    /// Parent replacement completed.
    ParentReplaced,
    /// Reconciliation phase is running.
    Reconciliation,
    /// Original parent returned.
    ParentReturned,
    /// A shadow parent was created.
    ShadowParentCreated,
    /// Lineage stabilized after changes.
    LineageStabilized,
}

// ── AdoptionEvent ─────────────────────────────────────────────────────────────

/// Runtime event emitted by the family adoption system.
#[derive(Debug, Clone)]
pub struct AdoptionEvent {
    /// Type of event emitted.
    pub event_type: AdoptionEventType,
    /// Child node involved in the event.
    pub child_id:   FamilyId,
    /// Current parent A id.
    pub parent_a_id: FamilyId,
    /// Current parent B id.
    pub parent_b_id: FamilyId,
    /// Event timestamp in milliseconds.
    pub timestamp:  u64,
    /// Visibility scope for the event.
    pub scope:      EventScope,
}
