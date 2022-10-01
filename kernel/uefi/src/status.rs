use core::{mem, result};

use struct_enum::struct_enum;

pub type Result<T> = result::Result<T, Status>;

const ERROR_BIT: usize = 1 << (mem::size_of::<usize>() * 8 - 1);

const fn err(code: usize) -> usize {
    code | ERROR_BIT
}

struct_enum! {
    #[must_use]
    pub struct Status: usize {
        SUCCESS = 0;

        WARN_UNKNOWN_GLYPH = 1;

        LOAD_ERROR = err(1);
        INVALID_PARAMETER = err(2);
        UNSUPPORTED = err(3);
        BUFFER_TOO_SMALL = err(5);
        OUT_OF_RESOURCES = err(9);
        END_OF_FILE = err(31);
    }
}

impl Status {
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
