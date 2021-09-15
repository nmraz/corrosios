use core::convert::TryFrom;
use core::{fmt, mem, slice};

use crate::Status;

#[derive(Debug, Clone, Copy)]
pub struct FromCharsWithNulError;

impl From<FromCharsWithNulError> for Status {
    fn from(_: FromCharsWithNulError) -> Self {
        Self::INVALID_PARAMETER
    }
}

#[repr(transparent)]
pub struct U16CStr([u16]);

impl U16CStr {
    pub fn from_chars_with_nul(slice: &[u16]) -> Result<&Self, FromCharsWithNulError> {
        let nul_pos = slice.iter().position(|&c| c == 0);

        if nul_pos.filter(|pos| pos + 1 == slice.len()).is_none() {
            return Err(FromCharsWithNulError);
        }

        Ok(unsafe { Self::from_chars_with_nul_unchecked(slice) })
    }

    /// # Safety
    ///
    /// Must be nul-terminated.
    pub unsafe fn from_ptr<'a>(ptr: *const u16) -> &'a Self {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        Self::from_chars_with_nul_unchecked(slice::from_raw_parts(ptr, len))
    }

    /// # Safety
    ///
    /// Must be nul-terminated and not contain any embedded nuls.
    pub unsafe fn from_chars_with_nul_unchecked(slice: &[u16]) -> &Self {
        mem::transmute(slice)
    }

    pub fn as_slice(&self) -> &[u16] {
        &self.0
    }

    pub fn as_ptr(&self) -> *const u16 {
        self.as_slice().as_ptr()
    }
}

impl fmt::Display for U16CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.as_slice() {
            char::try_from(c as u32).map_err(|_| fmt::Error)?.fmt(f)?;
        }
        Ok(())
    }
}
