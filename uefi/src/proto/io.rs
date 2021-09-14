use core::fmt;

use crate::types::U16CStr;
use crate::{Result, Status};

use super::{abi_call, unsafe_protocol, Protocol};

#[repr(C)]
pub struct SimpleTextOutputAbi {
    reset: unsafe extern "efiapi" fn(*mut Self, bool) -> Status,
    output_string: unsafe extern "efiapi" fn(*mut Self, *const u16) -> Status,
    test_string: unsafe extern "efiapi" fn(*mut Self, *const u16) -> Status,
    query_mode: *const (),
    set_mode: *const (),
    set_attribute: *const (),
    clear_screen: unsafe extern "efiapi" fn(*mut Self) -> Status,
    set_cursor_pos: *const (),
    enable_cursor: unsafe extern "efiapi" fn(*mut Self, bool) -> Status,
    mode: *const (),
}

unsafe_protocol! {
    SimpleTextOutput(SimpleTextOutputAbi,
        (0x387477c2, 0x69c7, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]));
}

impl SimpleTextOutput {
    pub fn reset(&mut self) -> Result<()> {
        unsafe { abi_call!(self, reset(false)) }.to_result()
    }

    /// # Safety
    ///
    /// Must be null-terminated.
    pub unsafe fn output_string_unchecked(&mut self, s: *const u16) -> Result<()> {
        abi_call!(self, output_string(s)).to_result()
    }

    pub fn output_u16_str(&mut self, s: &U16CStr) -> Result<()> {
        unsafe { self.output_string_unchecked(s.as_ptr()) }
    }

    pub fn output_str(&mut self, s: &str) -> Result<()> {
        const BUF_LEN: usize = 64;

        let mut buf = [0u16; BUF_LEN + 1];
        let mut i = 0;

        let mut status = Ok(());

        let mut putchar = |ch| {
            if i == BUF_LEN {
                status = unsafe { self.output_string_unchecked(buf.as_ptr()) };
                status.map_err(|_| ucs2::Error::BufferOverflow)?;

                buf.fill(0);
                i = 0;
            }

            buf[i] = ch;
            i += 1;

            Ok(())
        };

        let res = ucs2::encode_with(s, |ch| {
            if ch == b'\n' as u16 {
                putchar(b'\r' as u16)?;
            }
            putchar(ch)
        });

        status?;
        res.map_err(|_| Status::WARN_UNKNOWN_GLYPH)?;

        unsafe { self.output_string_unchecked(buf.as_ptr()) }
    }
}

impl fmt::Write for SimpleTextOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.output_str(s).map_err(|_| fmt::Error)
    }
}
