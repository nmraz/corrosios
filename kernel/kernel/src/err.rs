use core::alloc::AllocError;

use alloc::collections::TryReserveError;
use struct_enum::struct_enum;

struct_enum! {
    pub struct Error: u32 {
        INVALID_ARGUMENT = 1;
        INVALID_STATE = 2;
        BAD_ADDRESS = 3;
        OUT_OF_MEMORY = 4;
        RESOURCE_OVERLAP = 5;
        OUT_OF_RESOURCES = 6;
        NO_PERMS = 7;
    }
}

impl From<AllocError> for Error {
    fn from(_: AllocError) -> Self {
        Self::OUT_OF_MEMORY
    }
}

impl From<TryReserveError> for Error {
    fn from(_: TryReserveError) -> Self {
        Self::OUT_OF_MEMORY
    }
}

pub type Result<T> = core::result::Result<T, Error>;
