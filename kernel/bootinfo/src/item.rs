use struct_enum::struct_enum;

struct_enum! {
    pub struct MemoryKind: u32 {
        RESERVED = 0;
        USABLE = 1;
        FIRMWARE_BOOT = 2;
        FIRMWARE_RUNIME = 3;
        ACPI_TABLES = 4;
        UNUSABLE = 5;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRange {
    pub start_page: usize,
    pub page_count: usize,
    pub kind: MemoryKind,
}

struct_enum! {
    pub struct PixelFormat: u32 {
        RGB = 0;
        BGR = 1;
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Framebuffer {
    pub paddr: usize,
    pub size: usize,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub format: PixelFormat,
}
