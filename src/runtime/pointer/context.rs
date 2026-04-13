//! Pointer Context - tracks active pointer for PULL/PUSH operations
//!
//! The context stack enables nested GOTO blocks to properly restore
//! the previous pointer context when exiting.

use super::pointer::PointerId;

/// Active pointer context with stack for nested GOTO blocks
#[derive(Debug, Clone)]
pub struct PointerContext {
    /// Currently active pointer (None if no GOTO active)
    current: Option<PointerId>,
    /// Stack of previous contexts for nested GOTO
    stack: Vec<Option<PointerId>>,
}

impl PointerContext {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            current: None,
            stack: Vec::new(),
        }
    }

    /// Get the currently active pointer
    pub fn current(&self) -> Option<PointerId> {
        self.current
    }

    /// Set the active pointer (for GOTO statement)
    /// Pushes the old context onto the stack
    pub fn push_context(&mut self, ptr_id: PointerId) {
        self.stack.push(self.current);
        self.current = Some(ptr_id);
    }

    /// Pop the context stack (on GOTO block exit)
    /// Restores the previous active pointer
    pub fn pop_context(&mut self) -> Option<PointerId> {
        let old = self.current;
        self.current = self.stack.pop().flatten();
        old
    }

    /// Directly set context without pushing (for simple assignment)
    pub fn set_context(&mut self, ptr_id: Option<PointerId>) {
        self.current = ptr_id;
    }

    /// Clear all context (reset)
    pub fn clear(&mut self) {
        self.current = None;
        self.stack.clear();
    }

    /// Depth of the context stack
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if any pointer is active
    pub fn has_active(&self) -> bool {
        self.current.is_some()
    }
}

impl Default for PointerContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_push_pop() {
        let mut ctx = PointerContext::new();
        assert!(ctx.current().is_none());
        assert_eq!(ctx.depth(), 0);

        // Push first context
        ctx.push_context(1);
        assert_eq!(ctx.current(), Some(1));
        assert_eq!(ctx.depth(), 1);

        // Push nested context
        ctx.push_context(2);
        assert_eq!(ctx.current(), Some(2));
        assert_eq!(ctx.depth(), 2);

        // Pop back to first
        ctx.pop_context();
        assert_eq!(ctx.current(), Some(1));
        assert_eq!(ctx.depth(), 1);

        // Pop to none
        ctx.pop_context();
        assert!(ctx.current().is_none());
        assert_eq!(ctx.depth(), 0);
    }
}
