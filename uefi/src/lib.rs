#![feature(abi_efiapi, asm)]
#![feature(allocator_api)]
#![no_std]

pub use bootalloc::BootAlloc;
pub use status::{Result, Status};
pub use types::*;

pub mod proto;
pub mod table;

mod bootalloc;
mod status;
mod types;
