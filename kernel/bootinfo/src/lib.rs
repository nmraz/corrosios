#![no_std]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ItemKind(pub u32);

impl ItemKind {
    pub const CONTAINER: Self = Self(0xb007b081);
}

pub const ITEM_ALIGN: usize = 8;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ItemHeader {
    pub kind: ItemKind,
    pub payload_len: u32,
}
