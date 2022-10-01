use struct_enum::struct_enum;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Handle(pub(crate) *const ());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Guid(pub u32, pub u16, pub u16, pub [u8; 8]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

struct_enum! {
    pub struct MemoryType: u32 {
        RESERVED = 0;
        LOADER_CODE = 1;
        LOADER_DATA = 2;
        BOOT_SERVICES_CODE = 3;
        BOOT_SERVICES_DATA = 4;
        RUNTIME_SERVICES_CODE = 5;
        RUNTIME_SERVICES_DATA = 6;
        CONVENTIONAL = 7;
        UNUSABLE = 8;
        ACPI_RECLAIM = 9;
    }
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

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct ConfigTableEntry {
    pub guid: Guid,
    pub ptr: usize,
}
