// src/runtime/family/mod.rs
//! Pasta Family Object System
//!
//! Implements the OBJ.FAM family group model:
//!   OBJ.LST  — list family group
//!   OBJ.DICT — dictionary family group
//!   OBJ.TNSR — tensor family group
//!   OBJ.NRML — normal object family group
//!   OBJ.CSM  — custom (cross-type) family group
//!
//! Key concepts:
//! - Every non-primordial node has exactly two parents.
//! - Two immutable primordial grandparents anchor the entire universe.
//! - The Adoption State Machine (ASM) handles parent loss and reconciliation.
//! - DOES_PARENT_EXIST is the user-facing boolean health check.
//! - USE UNSAFE-READ / USE UNSAFE-WRITE gate global root-space events.

pub mod asm;
pub mod errors;
pub mod events;
pub mod family_node;
pub mod gc;
pub mod grandparent;
pub mod parent_check;
pub mod registry;
pub mod snapshots;
pub mod types;
mod tests;

pub use types::{
    FamilyId, NodeRole, ObjGroup, DeltaEntry, LineageSignature, MinimalSnapshot,
    ParentStatus, MissingParent, ReconciliationOutcome, EventScope,
    AdoptionEventType, AdoptionEvent, next_family_id,
};
pub use family_node::FamilyNode;
pub use grandparent::{GrandparentNode, Primordials, PRIMORDIAL_A_ID, PRIMORDIAL_B_ID};
pub use registry::FamilyRegistry;
pub use events::{FamilyEventBus, UnsafePermission};
pub use errors::{LineageError, LineageDiagnostic};
pub use parent_check::does_parent_exist;
pub use snapshots::{create_snapshot, restore_from_snapshot};
pub use gc::{gc_is_collectible, gc_collect};
