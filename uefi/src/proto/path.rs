use core::ptr::NonNull;

use crate::types::U16CStr;
use crate::{Result, Status};

use super::{unsafe_protocol, Protocol};

#[repr(C)]
pub struct DevicePathAbi {
    path_type: u8,
    sub_type: u8,
    length: u16,
}

unsafe_protocol! {
    DevicePath(DevicePathAbi,
        (0x09576e91, 0x6d3f, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]));
}

#[repr(C)]
pub struct DevicePathToTextAbi {
    device_node_to_text: unsafe extern "efiapi" fn(*const DevicePathAbi, bool, bool) -> *mut u16,
    device_path_to_text: unsafe extern "efiapi" fn(*const DevicePathAbi, bool, bool) -> *mut u16,
}

unsafe_protocol! {
    DevicePathToText(DevicePathToTextAbi,
        (0x8b843e20, 0x8132, 0x4852, [0x90, 0xcc, 0x55, 0x1a, 0x4e, 0x4a, 0x7f, 0x1c]));
}

impl DevicePathToText {
    pub fn device_path_to_text(
        &self,
        device_path: &DevicePath,
        display_only: bool,
        allow_shortcuts: bool,
    ) -> Result<NonNull<U16CStr>> {
        let p = unsafe {
            ((*self.abi()).device_path_to_text)(device_path.abi(), display_only, allow_shortcuts)
        };

        if p.is_null() {
            return Err(Status::OUT_OF_RESOURCES);
        }

        Ok(unsafe { NonNull::new_unchecked(U16CStr::from_ptr(p) as *const _ as *mut _) })
    }
}
