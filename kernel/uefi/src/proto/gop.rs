use crate::Status;

use super::{unsafe_protocol, Protocol};

#[repr(C)]
pub struct GraphicsOutputAbi {
    query_mode: unsafe extern "efiapi" fn(*mut Self, u32, usize, *mut u8) -> Status,
    set_mode: unsafe extern "efiapi" fn(*mut Self, u32) -> Status,
    blt: *const (),
    mode: *const CurrentModeAbi,
}

#[repr(C)]
struct CurrentModeAbi {
    max_mode: u32,
    mode: u32,
    info: *const ModeInfoAbi,
    info_size: usize,
    framebuffer_base: u64,
    framebuffer_size: usize,
}

#[repr(C)]
struct ModeInfoAbi {
    version: u32,
    hres: u32,
    vres: u32,
    pixel_format: u32,
    pixel_bitmask: PixelBitmask,
    pixels_per_scanline: u32,
}

unsafe_protocol! {
    GraphicsOutput(GraphicsOutputAbi, "9042a9de-23dc-4a38-96fb-7aded080516a");
}

#[derive(Debug, Clone, Copy)]
pub struct CurrentMode {
    pub info: ModeInfo,
    pub framebuffer: Option<FramebufferInfo>,
}

#[derive(Debug, Clone, Copy)]
pub struct FramebufferInfo {
    pub base: u64,
    pub size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ModeInfo {
    pub hres: u32,
    pub vres: u32,
    pub pixels_per_scanline: u32,
    pub pixel_format: PixelFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb,
    Bgr,
    Bitmask(PixelBitmask),
    Unknown,
}

impl PixelFormat {
    pub fn has_framebuffer(&self) -> bool {
        self != &Self::Unknown
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct PixelBitmask {
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub reserved_mask: u32,
}

impl GraphicsOutput {
    pub fn current_mode(&self) -> CurrentMode {
        let mode = unsafe { &*(*self.abi()).mode };
        let info = unsafe { mode_info_from_abi(&*mode.info) };

        let framebuffer = info
            .pixel_format
            .has_framebuffer()
            .then_some(FramebufferInfo {
                base: mode.framebuffer_base,
                size: mode.framebuffer_size,
            });

        CurrentMode { info, framebuffer }
    }
}

const PIXEL_FORMAT_RGB: u32 = 0;
const PIXEL_FORMAT_BGR: u32 = 1;
const PIXEL_FORMAT_BITMASK: u32 = 2;

fn mode_info_from_abi(abi: &ModeInfoAbi) -> ModeInfo {
    let pixel_format = match abi.pixel_format {
        PIXEL_FORMAT_RGB => PixelFormat::Rgb,
        PIXEL_FORMAT_BGR => PixelFormat::Bgr,
        PIXEL_FORMAT_BITMASK => PixelFormat::Bitmask(abi.pixel_bitmask),
        _ => PixelFormat::Unknown,
    };

    ModeInfo {
        hres: abi.hres,
        vres: abi.vres,
        pixels_per_scanline: abi.pixels_per_scanline,
        pixel_format,
    }
}
