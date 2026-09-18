// src/runtime/family/errors.rs
//! Error types for the Pasta Family Object System.

use std::fmt;
use crate::runtime::family::types::FamilyId;

/// Errors emitted while maintaining or repairing family lineage.
#[derive(Debug)]
pub enum LineageError {
    /// Generic lineage failure for a node.
    LineageFailure      {
        /// Node whose lineage update failed.
        node_id: FamilyId,
        /// Human-readable failure reason.
        reason: String,
    },
    /// Timed out while waiting for a parent slot to be filled.
    AdoptionTimeout     {
        /// Node waiting for adoption to complete.
        node_id: FamilyId,
        /// Parent slot that timed out.
        slot: String,
    },
    /// Failed while reconciling lineage state.
    ReconciliationError {
        /// Node whose lineage reconciliation failed.
        node_id: FamilyId,
        /// Human-readable reconciliation failure reason.
        reason: String,
    },
    /// Could not resolve the requested parent id.
    ParentResolutionError {
        /// Node whose parent lookup failed.
        node_id: FamilyId,
        /// Parent id that could not be resolved.
        parent_id: FamilyId,
    },
    /// Child and parent groups are incompatible.
    GroupTypeMismatch   {
        /// Child group type.
        child_group: String,
        /// Parent group type.
        parent_group: String,
    },
    /// Requested action requires a permission the caller lacks.
    PermissionDenied    {
        /// Action that was denied.
        action: String,
        /// Permission required to perform the action.
        required_permission: String,
    },
}

impl fmt::Display for LineageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LineageError::LineageFailure { node_id, reason } =>
                write!(f, "Lineage failure for node {node_id}: {reason}"),
            LineageError::AdoptionTimeout { node_id, slot } =>
                write!(f, "Adoption timeout for node {node_id} waiting for parent slot {slot}"),
            LineageError::ReconciliationError { node_id, reason } =>
                write!(f, "Reconciliation error for node {node_id}: {reason}"),
            LineageError::ParentResolutionError { node_id, parent_id } =>
                write!(f, "Parent resolution error for node {node_id}: cannot resolve parent {parent_id}"),
            LineageError::GroupTypeMismatch { child_group, parent_group } =>
                write!(f, "Type mismatch: child group {child_group} cannot have parent of group {parent_group}"),
            LineageError::PermissionDenied { action, required_permission } =>
                write!(f, "Permission denied: {action} requires {required_permission}"),
        }
    }
}

impl std::error::Error for LineageError {}

/// Diagnostic artifact emitted on serious failures.
#[derive(Debug)]
pub struct LineageDiagnostic {
    /// Primary lineage error.
    pub error:     LineageError,
    /// Known ancestry chain at the time of failure.
    pub ancestry:  Vec<FamilyId>,
    /// State-machine snapshot as text.
    pub asm_state: String,
    /// Relevant recent lineage/family events.
    pub event_log: Vec<String>,
}

impl LineageDiagnostic {
    /// Construct a new lineage diagnostic artifact.
    pub fn new(error: LineageError, ancestry: Vec<FamilyId>, asm_state: &str, event_log: Vec<String>) -> Self {
        LineageDiagnostic { error, ancestry, asm_state: asm_state.to_string(), event_log }
    }
}
