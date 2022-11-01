#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use core::mem;

use struct_enum::struct_enum;

pub mod builder;
pub mod item;
pub mod view;

#[derive(Debug, Clone, Copy)]
pub enum Error {
    BadSize,
    BadAlign,
}

pub const ITEM_ALIGN: usize = 8;

struct_enum! {
    pub struct ItemKind: u32 {
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

const _: () = {
    assert!(mem::align_of::<ItemHeader>() <= ITEM_ALIGN);
    assert!(mem::size_of::<ItemHeader>() == ITEM_ALIGN);
};
