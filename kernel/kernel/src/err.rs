use core::alloc::AllocError;

use struct_enum::struct_enum;

struct_enum! {
    pub struct Error: u32 {
        OUT_OF_MEMORY = 1;
        INVALID_STATE = 2;
        RESOURCE_IN_USE = 3;
    }
}

impl From<AllocError> for Error {
    fn from(_: AllocError) -> Self {
        Self::OUT_OF_MEMORY
    }
}

pub type Result<T> = core::result::Result<T, Error>;
