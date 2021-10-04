use core::{mem, result};

pub type Result<T> = result::Result<T, Status>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
#[must_use]
pub struct Status(pub usize);

const ERROR_BIT: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

impl Status {
    pub const SUCCESS: Self = Self(0);

    pub const INVALID_PARAMETER: Self = Self(2 | ERROR_BIT);
    pub const BUFFER_TOO_SMALL: Self = Self(5 | ERROR_BIT);
    pub const OUT_OF_RESOURCES: Self = Self(9 | ERROR_BIT);
    pub const END_OF_FILE: Self = Self(31 | ERROR_BIT);

    pub const WARN_UNKNOWN_GLYPH: Self = Self(1);

    pub fn is_err(self) -> bool {
        self.0 & ERROR_BIT != 0
    }

    pub fn is_success(self) -> bool {
        self == Self::SUCCESS
    }

    pub fn is_warn(self) -> bool {
        !self.is_success() && !self.is_err()
    }

    pub fn to_result(self) -> Result<()> {
        if self.is_err() {
            Err(self)
        } else {
            Ok(())
        }
    }
}
