#![feature(abi_efiapi, asm)]
#![feature(allocator_api)]
#![no_std]

// Allow proc macros referencing `::uefi` to work within this crate
extern crate self as uefi;

pub use uefi_macros::{guid, u16cstr};

pub use bootalloc::BootAlloc;
pub use cstr::*;
pub use status::{Result, Status};
pub use types::*;

pub mod proto;
pub mod table;

mod bootalloc;
mod cstr;
mod status;
mod types;
