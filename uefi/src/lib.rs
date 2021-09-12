#![feature(abi_efiapi, asm)]
#![feature(allocator_api)]
#![no_std]

pub use bootalloc::BootAlloc;
pub use status::{Result, Status};

pub mod proto;
pub mod table;
pub mod types;

mod bootalloc;
mod status;
