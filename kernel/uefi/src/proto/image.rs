use crate::types::{Handle, MemoryType};
use crate::Status;

use super::path::{DevicePath, DevicePathAbi};
use super::{unsafe_protocol, Protocol, ProtocolHandle};

#[repr(C)]
pub struct LoadedImageAbi {
    revision: u32,
    parent_handle: Handle,
    system_table: *const (),
    device_handle: Handle,
    file_path: *mut DevicePathAbi,
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
    LoadedImage(LoadedImageAbi, "5b1b31a1-9562-11d2-8e3f-00a0c969723b");
}

impl LoadedImage {
    pub fn device_handle(&self) -> Handle {
        unsafe { (*self.abi()).device_handle }
    }

    pub fn file_path(&self) -> ProtocolHandle<'_, DevicePath> {
        unsafe { ProtocolHandle::from_abi((*self.abi()).file_path) }
    }

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
