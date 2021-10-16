use crate::Status;

#[repr(C)]
pub struct GraphicsOutputAbi {
    query_mode: unsafe extern "efiapi" fn(*mut Self, u32, usize, *mut u8) -> Status,
    set_mode: unsafe extern "efiapi" fn(*mut Self, u32) -> Status,
    blt: *const (),
    mode: *const CurrentMode,
}

#[repr(C)]
struct CurrentMode {
    max_mode: u32,
    mode: u32,
    info: *const ModeInfo,
    info_size: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct ModeInfo {
    version: u32,
    hres: u32,
    vres: u32,
    pixel_format: u32,
    pixel_bitmask: PixelBitmask,
    pixels_per_scanline: u32,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct PixelBitmask {
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
}
