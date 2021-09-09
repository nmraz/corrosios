use core::convert::TryFrom;
use core::{fmt, mem, slice};

#[repr(transparent)]
pub struct Handle(*const ());

#[repr(transparent)]
pub struct U16CStr([u16]);

impl U16CStr {
    /// # Safety
    ///
    /// Must be null-terminated.
    pub unsafe fn from_ptr<'a>(ptr: *const u16) -> &'a Self {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let data = slice::from_raw_parts(ptr, len);
        mem::transmute(data)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryMapKey(pub(crate) usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryType(pub u32);

impl MemoryType {
    pub const RESERVED: Self = Self(0);
    pub const LOADER_CODE: Self = Self(1);
    pub const LOADER_DATA: Self = Self(2);
    pub const BOOT_SERVICES_CODE: Self = Self(3);
    pub const BOOT_SERVICES_DATA: Self = Self(4);
    pub const RUNTIME_SERVICES_CODE: Self = Self(5);
    pub const RUNTIME_SERVICES_DATA: Self = Self(6);
    pub const CONVENTIONAL: Self = Self(7);
    pub const UNUSABLE: Self = Self(8);
}

#[repr(C)]
pub struct MemoryDescriptor {
    pub mem_type: MemoryType,
    pub phys_start: u64,
    pub virt_start: u64,
    pub page_count: u64,
    pub attr: u64,
}
