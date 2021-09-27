#![feature(abi_efiapi, asm)]
#![feature(allocator_api)]
#![no_std]

pub use uefi_macros::u16cstr;

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
