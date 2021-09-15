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
    pub fn from_u16s_with_nul(slice: &[u16]) -> Result<&Self, FromCharsWithNulError> {
        let nul_pos = slice.iter().position(|&c| c == 0);

        if nul_pos.filter(|pos| pos + 1 == slice.len()).is_none() {
            return Err(FromCharsWithNulError);
        }

        Ok(unsafe { Self::from_u16s_with_nul_unchecked(slice) })
    }

    /// # Safety
    ///
    /// Must be nul-terminated.
    pub unsafe fn from_ptr<'a>(ptr: *const u16) -> &'a Self {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        Self::from_u16s_with_nul_unchecked(slice::from_raw_parts(ptr, len + 1))
    }

    /// # Safety
    ///
    /// Must be nul-terminated and not contain any embedded nuls.
    pub unsafe fn from_u16s_with_nul_unchecked(slice: &[u16]) -> &Self {
        mem::transmute(slice)
    }

    pub fn to_u16s_with_nul(&self) -> &[u16] {
        &self.0
    }

    pub fn to_u16s(&self) -> &[u16] {
        let u16s = self.to_u16s_with_nul();
        &u16s[..u16s.len() - 1]
    }

    pub fn as_ptr(&self) -> *const u16 {
        self.to_u16s_with_nul().as_ptr()
    }
}

impl fmt::Display for U16CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.to_u16s() {
            char::try_from(c as u32).map_err(|_| fmt::Error)?.fmt(f)?;
        }
        Ok(())
    }
}
