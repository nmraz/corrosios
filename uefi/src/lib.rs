#![feature(abi_efiapi, asm)]
#![no_std]

pub use status::{Result, Status};

pub mod proto;
pub mod table;
pub mod types;

mod status;
