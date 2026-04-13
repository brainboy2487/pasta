//! GC Integration for Pointer System
//!
//! Hooks into the Saucey Strainer GC for automatic pointer lifecycle management.
//! 
//! This module provides:
//! - Allocation tracking (notifies GC of new pointers)
//! - Temporary pointer lifetime (auto-free on scope exit)
//! - Free-after-use semantics
//! - Dead pointer sweep

use super::registry::PointerRegistry;
use super::pointer::PointerId;
use crate::runtime::strainer::{Strainer, GcRef};
use crate::interpreter::environment::Value;
use std::collections::HashSet;

/// GC tracking state for pointers
pub struct PointerGcTracker {
    /// Set of pointer IDs allocated in the current scope
    scope_allocations: Vec<HashSet<PointerId>>,
    /// Set of pointer IDs that are "rooted" (should not be auto-freed)
    rooted: HashSet<PointerId>,
    /// Mapping from PointerId to GcRef for strainer integration
    gc_refs: std::collections::HashMap<PointerId, GcRef>,
}

impl PointerGcTracker {
    /// Create a new GC tracker
    pub fn new() -> Self {
        Self {
            scope_allocations: vec![HashSet::new()],
            rooted: HashSet::new(),
            gc_refs: std::collections::HashMap::new(),
        }
    }

    // --- Scope management ---

    /// Enter a new scope (e.g., DO-block, GOTO block)
    pub fn enter_scope(&mut self) {
        self.scope_allocations.push(HashSet::new());
    }

    /// Exit the current scope, returning pointers allocated in that scope
    /// that should be freed (i.e., temporary and not rooted)
    pub fn exit_scope(&mut self) -> Vec<PointerId> {
        if self.scope_allocations.len() > 1 {
            let scope = self.scope_allocations.pop().unwrap_or_default();
            // Return unrooted pointers from this scope
            scope.into_iter()
                .filter(|id| !self.rooted.contains(id))
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Current scope depth
    pub fn scope_depth(&self) -> usize {
        self.scope_allocations.len()
    }

    // --- Allocation tracking ---

    /// Notify tracker that a pointer was allocated
    pub fn track_allocation(&mut self, id: PointerId) {
        if let Some(scope) = self.scope_allocations.last_mut() {
            scope.insert(id);
        }
    }

    /// Mark a pointer as rooted (will not be auto-freed on scope exit)
    pub fn root(&mut self, id: PointerId) {
        self.rooted.insert(id);
    }

    /// Unroot a pointer (can be auto-freed on scope exit)
    pub fn unroot(&mut self, id: PointerId) {
        self.rooted.remove(&id);
    }

    /// Check if a pointer is rooted
    pub fn is_rooted(&self, id: PointerId) -> bool {
        self.rooted.contains(&id)
    }

    // --- Strainer integration ---

    /// Associate a pointer with a GcRef
    pub fn associate_gc_ref(&mut self, ptr_id: PointerId, gc_ref: GcRef) {
        self.gc_refs.insert(ptr_id, gc_ref);
    }

    /// Get the GcRef for a pointer
    pub fn get_gc_ref(&self, ptr_id: PointerId) -> Option<GcRef> {
        self.gc_refs.get(&ptr_id).copied()
    }

    /// Remove GcRef association (on pointer free)
    pub fn remove_gc_ref(&mut self, ptr_id: PointerId) -> Option<GcRef> {
        self.gc_refs.remove(&ptr_id)
    }

    // --- Reporting ---

    /// Get all tracked allocations across all scopes
    pub fn all_allocations(&self) -> Vec<PointerId> {
        self.scope_allocations.iter()
            .flat_map(|s| s.iter().copied())
            .collect()
    }

    /// Get count of allocations in current scope
    pub fn current_scope_count(&self) -> usize {
        self.scope_allocations.last()
            .map(|s| s.len())
            .unwrap_or(0)
    }
}

impl Default for PointerGcTracker {
    fn default() -> Self {
        Self::new()
    }
}

// --- Public GC hook functions ---

/// Free all temporary pointers in a registry that were allocated in the current scope
pub fn gc_free_scope_temporaries(
    registry: &mut PointerRegistry,
    tracker: &mut PointerGcTracker,
) -> Vec<PointerId> {
    let to_free = tracker.exit_scope();
    
    for &id in &to_free {
        // Mark as dead in registry
        registry.kill(id);
        // Remove GC association
        tracker.remove_gc_ref(id);
    }
    
    to_free
}

/// Sweep dead pointers from registry and clean up GC associations
pub fn gc_sweep_dead_pointers(
    registry: &mut PointerRegistry,
    _tracker: &mut PointerGcTracker,
) -> usize {
    // Get dead pointer IDs before sweep
    let _dead_ids: Vec<PointerId> = {
        // Access internal state to find dead pointers
        // For now, sweep_dead returns count, so we track separately
        Vec::new() // Will be populated by sweep
    };
    
    // Sweep dead from registry
    let count = registry.sweep_dead();
    
    // Note: GC refs are already cleaned up when pointers are freed
    count
}

/// Notify GC of a new pointer allocation
pub fn gc_notify_alloc(
    tracker: &mut PointerGcTracker,
    id: PointerId,
    strainer: Option<&mut Strainer>,
) {
    // Track in current scope
    tracker.track_allocation(id);
    
    // Optionally register with strainer
    if let Some(gc) = strainer {
        let gc_ref = gc.allocate(Value::Pointer(id));
        tracker.associate_gc_ref(id, gc_ref);
    }
}

/// Notify GC of a pointer being freed
pub fn gc_notify_free(
    tracker: &mut PointerGcTracker,
    id: PointerId,
    strainer: Option<&mut Strainer>,
) {
    // Unroot if rooted
    tracker.unroot(id);
    
    // Remove GC association and unregister from strainer
    if let Some(gc_ref) = tracker.remove_gc_ref(id) {
        if let Some(gc) = strainer {
            gc.unregister_root(gc_ref);
        }
    }
}

/// Root a pointer so it won't be auto-freed
pub fn gc_root_pointer(
    tracker: &mut PointerGcTracker,
    id: PointerId,
    strainer: Option<&mut Strainer>,
) {
    tracker.root(id);
    
    // Also register as GC root in strainer
    if let Some(gc_ref) = tracker.get_gc_ref(id) {
        if let Some(gc) = strainer {
            gc.register_root(gc_ref);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_scope_tracking() {
        let mut tracker = PointerGcTracker::new();
        
        // Allocate in root scope
        tracker.track_allocation(1);
        assert_eq!(tracker.current_scope_count(), 1);
        
        // Enter new scope
        tracker.enter_scope();
        tracker.track_allocation(2);
        tracker.track_allocation(3);
        assert_eq!(tracker.current_scope_count(), 2);
        assert_eq!(tracker.scope_depth(), 2);
        
        // Exit scope - should return unrooted allocations
        let freed = tracker.exit_scope();
        assert_eq!(freed.len(), 2);
        assert!(freed.contains(&2));
        assert!(freed.contains(&3));
        
        // Back to root scope
        assert_eq!(tracker.scope_depth(), 1);
        assert_eq!(tracker.current_scope_count(), 1);
    }
    
    #[test]
    fn test_rooting() {
        let mut tracker = PointerGcTracker::new();
        
        tracker.enter_scope();
        tracker.track_allocation(1);
        tracker.track_allocation(2);
        
        // Root pointer 1
        tracker.root(1);
        assert!(tracker.is_rooted(1));
        assert!(!tracker.is_rooted(2));
        
        // Exit scope - only unrooted should be freed
        let freed = tracker.exit_scope();
        assert_eq!(freed.len(), 1);
        assert!(freed.contains(&2));
        assert!(!freed.contains(&1));
    }
}
