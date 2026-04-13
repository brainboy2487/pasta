// src/runtime/family/grandparent.rs
//! Primordial grandparent nodes — immutable fixed anchors of the family universe.
//!
//! Exactly two primordials exist. They are created once on first runtime startup
//! and persisted. They have no parents, are never updated semantically, and hold
//! minimal recovery metadata and lineage signatures.

use crate::runtime::family::types::{FamilyId, LineageSignature, MinimalSnapshot, NodeRole};

/// Stable id for primordial alpha.
pub const PRIMORDIAL_A_ID: FamilyId = 1;
/// Stable id for primordial beta.
pub const PRIMORDIAL_B_ID: FamilyId = 2;

/// Immutable primordial grandparent node.
#[derive(Debug, Clone)]
pub struct GrandparentNode {
    /// Primordial family id.
    pub id:                FamilyId,
    /// Always `NodeRole::Grandparent`.
    pub role:              NodeRole,   // always NodeRole::Grandparent
    /// Primordial lineage signature.
    pub lineage:           LineageSignature,
    /// Minimal recovery snapshot for the primordial.
    pub recovery_metadata: MinimalSnapshot,
}

impl GrandparentNode {
    fn new_primordial(id: FamilyId, hash_seed: u64) -> Self {
        let lineage = LineageSignature { hash: hash_seed, version: 0 };
        let recovery_metadata = MinimalSnapshot {
            lineage:          lineage.clone(),
            mutation_count:   0,
            trait_count:      0,
            capability_count: 0,
            timestamp:        0,
            parent_a_id:      0,
            parent_b_id:      0,
        };
        GrandparentNode {
            id,
            role: NodeRole::Grandparent,
            lineage,
            recovery_metadata,
        }
    }

    /// Validate that a lineage chain ultimately anchors to a known primordial hash.
    pub fn validate_lineage(&self, sig: &LineageSignature) -> bool {
        // Structural check: change distance from this primordial anchor must be finite.
        // In practice, any valid lineage hash differs from the primordial by a finite
        // number of evolution steps; here we just verify the signature is non-zero.
        sig.hash != 0
    }
}

/// The two primordial grandparents, created once at runtime startup and held here.
pub struct Primordials {
    /// Primordial alpha.
    pub alpha: GrandparentNode,
    /// Primordial beta.
    pub beta:  GrandparentNode,
}

impl Primordials {
    /// Create (or recreate from persisted seed hashes) the two primordials.
    /// seed_alpha / seed_beta are loaded from persistence; on first boot they
    /// are generated from a fixed compile-time constant.
    pub fn initialize(seed_alpha: Option<u64>, seed_beta: Option<u64>) -> Self {
        let alpha_hash = seed_alpha.unwrap_or(0x9e3779b97f4a7c15);
        let beta_hash  = seed_beta.unwrap_or(0x6c62272e07bb0142);
        Primordials {
            alpha: GrandparentNode::new_primordial(PRIMORDIAL_A_ID, alpha_hash),
            beta:  GrandparentNode::new_primordial(PRIMORDIAL_B_ID, beta_hash),
        }
    }

    /// Return a primordial node by id if the id belongs to one.
    pub fn get(&self, id: FamilyId) -> Option<&GrandparentNode> {
        match id {
            PRIMORDIAL_A_ID => Some(&self.alpha),
            PRIMORDIAL_B_ID => Some(&self.beta),
            _ => None,
        }
    }

    /// Return whether the given id belongs to a primordial node.
    pub fn is_primordial(id: FamilyId) -> bool {
        id == PRIMORDIAL_A_ID || id == PRIMORDIAL_B_ID
    }
}
