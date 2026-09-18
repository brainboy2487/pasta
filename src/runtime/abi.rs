//! Stable ABI-facing value definitions for compiled Pasta artifacts.
//!
//! This module defines the first C-compatible runtime boundary for the native
//! compiler track. The ABI intentionally exposes only user-visible language
//! value kinds and hides interpreter-only internals behind runtime-managed
//! handles.

/// Opaque runtime-managed handle kinds used by the compiled ABI.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PHandleKind {
    /// UTF-8 string storage owned by the runtime.
    String = 1,
    /// Heterogeneous Pasta list storage owned by the runtime.
    List = 2,
    /// Key-value Pasta dictionary storage owned by the runtime.
    Dict = 3,
    /// Tensor storage owned by the runtime.
    Tensor = 4,
}

/// Stable top-level value tags for the compiled Pasta ABI.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PValueTag {
    /// No value / null-like sentinel.
    None = 0,
    /// Inline boolean scalar.
    Bool = 1,
    /// Inline numeric scalar (`f64`).
    Number = 2,
    /// Runtime-managed opaque heap handle.
    Handle = 3,
    /// Family node identifier plus mutability flag.
    FamilyNode = 4,
    /// Pointer subsystem identifier.
    Pointer = 5,
}

/// Inline ABI representation of a family-node handle.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PFamilyNodeValue {
    /// Runtime family node ID.
    pub id: u64,
    /// Whether the node is mutable.
    pub mutable_flag: u8,
    /// Reserved padding for stable layout/alignment.
    pub reserved: [u8; 7],
}

impl PFamilyNodeValue {
    /// Create a family-node ABI payload from an ID and mutability flag.
    pub const fn new(id: u64, mutable_flag: bool) -> Self {
        Self {
            id,
            mutable_flag: if mutable_flag { 1 } else { 0 },
            reserved: [0; 7],
        }
    }
}

/// Tagged payload storage for a [`PValue`].
#[repr(C)]
#[derive(Clone, Copy)]
pub union PValueData {
    /// Inline boolean scalar encoded as `0` or `1`.
    pub boolean: u8,
    /// Inline numeric scalar.
    pub number: f64,
    /// Runtime-managed opaque handle ID.
    pub handle_id: u64,
    /// Family-node payload.
    pub family_node: PFamilyNodeValue,
    /// Pointer subsystem ID.
    pub pointer_id: u64,
}

impl core::fmt::Debug for PValueData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("<pvalue-data>")
    }
}

/// C-compatible compiled/runtime ABI value.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PValue {
    /// Discriminator for the active payload.
    pub tag: PValueTag,
    /// Handle subtype when `tag == PValueTag::Handle`, otherwise `0`.
    pub handle_kind: u32,
    /// Payload storage.
    pub data: PValueData,
}

impl PValue {
    /// Construct a `None` ABI value.
    pub const fn none() -> Self {
        Self {
            tag: PValueTag::None,
            handle_kind: 0,
            data: PValueData { handle_id: 0 },
        }
    }

    /// Construct a boolean ABI value.
    pub const fn boolean(value: bool) -> Self {
        Self {
            tag: PValueTag::Bool,
            handle_kind: 0,
            data: PValueData {
                boolean: if value { 1 } else { 0 },
            },
        }
    }

    /// Construct a numeric ABI value.
    pub const fn number(value: f64) -> Self {
        Self {
            tag: PValueTag::Number,
            handle_kind: 0,
            data: PValueData { number: value },
        }
    }

    /// Construct an opaque runtime-handle ABI value.
    pub const fn handle(kind: PHandleKind, handle_id: u64) -> Self {
        Self {
            tag: PValueTag::Handle,
            handle_kind: kind as u32,
            data: PValueData { handle_id },
        }
    }

    /// Construct a family-node ABI value.
    pub const fn family_node(id: u64, mutable_flag: bool) -> Self {
        Self {
            tag: PValueTag::FamilyNode,
            handle_kind: 0,
            data: PValueData {
                family_node: PFamilyNodeValue::new(id, mutable_flag),
            },
        }
    }

    /// Construct a pointer ABI value.
    pub const fn pointer(pointer_id: u64) -> Self {
        Self {
            tag: PValueTag::Pointer,
            handle_kind: 0,
            data: PValueData { pointer_id },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pvalue_layout_is_stable_for_ffi() {
        assert_eq!(core::mem::size_of::<PFamilyNodeValue>(), 16);
        assert_eq!(core::mem::size_of::<PValue>(), 24);
        assert_eq!(core::mem::align_of::<PValue>(), 8);
    }

    #[test]
    fn constructors_set_expected_tags() {
        let none = PValue::none();
        assert_eq!(none.tag, PValueTag::None);

        let boolean = PValue::boolean(true);
        assert_eq!(boolean.tag, PValueTag::Bool);
        assert_eq!(boolean.handle_kind, 0);

        let number = PValue::number(42.0);
        assert_eq!(number.tag, PValueTag::Number);

        let handle = PValue::handle(PHandleKind::String, 9);
        assert_eq!(handle.tag, PValueTag::Handle);
        assert_eq!(handle.handle_kind, PHandleKind::String as u32);

        let family = PValue::family_node(7, true);
        assert_eq!(family.tag, PValueTag::FamilyNode);

        let pointer = PValue::pointer(88);
        assert_eq!(pointer.tag, PValueTag::Pointer);
    }
}
