use crate::types::{Handle, MemoryType};
use crate::Status;

use super::{unsafe_protocol, Protocol};

#[repr(C)]
pub struct LoadedImageAbi {
    revision: u32,
    parent_handle: Handle,
    system_table: *const (),
    device_handle: Handle,
    file_path: *const (), // TODO
    reserved: *const (),
    load_options_size: u32,
    load_options: *const (),
    image_base: *const (),
    image_size: u64,
    code_type: MemoryType,
    data_type: MemoryType,
    unload: unsafe extern "efiapi" fn(Handle) -> Status,
}

unsafe_protocol! {
    LoadedImage(LoadedImageAbi,
        (0x5B1B31A1, 0x9562, 0x11d2, [0x8E, 0x3F, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B]));
}

impl LoadedImage {
    pub fn image_base(&self) -> *const () {
        unsafe { (*self.abi()).image_base }
    }

    pub fn image_size(&self) -> u64 {
        unsafe { (*self.abi()).image_size }
    }

    pub fn code_type(&self) -> MemoryType {
        unsafe { (*self.abi()).code_type }
    }

    pub fn data_type(&self) -> MemoryType {
        unsafe { (*self.abi()).data_type }
    }
}
