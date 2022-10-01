#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use struct_enum::struct_enum;

pub mod builder;
pub mod item;
pub mod view;

pub const ITEM_ALIGN: usize = 8;

struct_enum! {
    pub struct ItemKind: u32 {
        CONTAINER = 0xb007b081;
        EFI_SYSTEM_TABLE = 1;
        MEMORY_MAP = 2;
        FRAMEBUFFER = 3;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ItemHeader {
    pub kind: ItemKind,
    pub payload_len: u32,
}
