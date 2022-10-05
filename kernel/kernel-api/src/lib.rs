#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]

use struct_enum::struct_enum;

struct_enum! {
    pub struct Error: u32 {
        OUT_OF_MEMORY = 1;
        INVALID_STATE = 2;
        RESOURCE_IN_USE = 3;
    }
}
