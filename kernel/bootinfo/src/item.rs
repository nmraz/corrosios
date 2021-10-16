#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryKind(pub u32);

impl MemoryKind {
    pub const RESERVED: Self = Self(0);
    pub const USABLE: Self = Self(1);
    pub const FIRMWARE: Self = Self(2);
    pub const ACPI_TABLES: Self = Self(3);
    pub const UNUSABLE: Self = Self(4);
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct MemoryRange {
    pub start_page: usize,
    pub page_count: usize,
    pub kind: MemoryKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PixelFormat(pub u32);

impl PixelFormat {
    pub const RGB: Self = Self(0);
    pub const BGR: Self = Self(1);
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
