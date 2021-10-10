#![no_std]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod builder;
pub mod view;

pub const ITEM_ALIGN: usize = 8;

pub fn align_item_offset(off: usize) -> usize {
    (off as usize + ITEM_ALIGN - 1) & 0usize.wrapping_sub(ITEM_ALIGN)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ItemKind(pub u32);

impl ItemKind {
    pub const CONTAINER: Self = Self(0xb007b081);
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ItemHeader {
    pub kind: ItemKind,
    pub payload_len: u32,
}
