#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Handle(pub(crate) *const ());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Guid(pub u32, pub u16, pub u16, pub [u8; 8]);

#[repr(C)]
pub struct Timestamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
    pub pad1: u8,
    pub nanosecond: u32,
    pub timezone: i16,
    pub daylight: u8,
    pub pad2: u8,
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryDescriptor {
    pub mem_type: MemoryType,
    pub phys_start: u64,
    pub virt_start: u64,
    pub page_count: u64,
    pub attr: u64,
}
