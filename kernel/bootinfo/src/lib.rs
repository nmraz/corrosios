#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

pub mod builder;
pub mod item;
pub mod view;

pub const ITEM_ALIGN: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ItemKind(pub u32);

impl ItemKind {
    pub const CONTAINER: Self = Self(0xb007b081);
    pub const EFI_SYSTEM_TABLE: Self = Self(1);
    pub const MEMORY_MAP: Self = Self(2);
    pub const FRAMEBUFFER: Self = Self(3);
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ItemHeader {
    pub kind: ItemKind,
    pub payload_len: u32,
}
