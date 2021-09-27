use core::ptr::NonNull;
use core::{iter, mem, slice};

use crate::{Result, Status, U16CStr};

use super::{unsafe_protocol, Protocol};

#[repr(C, packed)]
struct DeviceNodeHeaderAbi {
    node_type: u8,
    sub_type: u8,
    length: u16,
}

#[repr(transparent)]
pub struct DevicePathAbi(DeviceNodeHeaderAbi);

unsafe_protocol! {
    DevicePath(DevicePathAbi, "09576e91-6d3f-11d2-8e39-00a0c969723b");
}

impl DevicePath {
    pub fn nodes(&self) -> impl Iterator<Item = DeviceNode<'_>> + Clone {
        // Safety: ABI pointer is valid.
        let init = DeviceNode(unsafe { &*(self.abi() as *const DeviceNodeHeaderAbi) });

        iter::successors(Some(init), |cur| {
            if cur.node_type() == DeviceNode::TYPE_END
                && cur.sub_type() == DeviceNode::SUB_TYPE_END_ENTIRE
            {
                None
            } else {
                // Safety: `length` bytes ahead there should be another device node header.
                let next =
                    unsafe { cur.ptr().add(cur.0.length as usize) } as *const DeviceNodeHeaderAbi;
                Some(DeviceNode(unsafe { &*next }))
            }
        })
    }
}

#[derive(Clone, Copy)]
pub struct DeviceNode<'a>(&'a DeviceNodeHeaderAbi);

impl<'a> DeviceNode<'a> {
    pub const TYPE_HARDWARE: u8 = 0x1;
    pub const TYPE_ACPI: u8 = 0x2;
    pub const TYPE_MESSAGING: u8 = 0x3;
    pub const TYPE_MEDIA: u8 = 0x4;
    pub const TYPE_BIOS: u8 = 0x5;
    pub const TYPE_END: u8 = 0x7f;

    pub const SUB_TYPE_END_ENTIRE: u8 = 0xff;
    pub const SUB_TYPE_END_DEVICE: u8 = 0x1;

    fn ptr(&self) -> *const u8 {
        self.0 as *const _ as *const u8
    }

    pub fn node_type(&self) -> u8 {
        self.0.node_type
    }

    pub fn sub_type(&self) -> u8 {
        self.0.sub_type
    }

    pub fn data(&self) -> &'a [u8] {
        let full_length = self.0.length as usize;
        assert!(full_length >= mem::size_of::<DeviceNodeHeaderAbi>());

        let length = full_length - mem::size_of::<DeviceNodeHeaderAbi>();
        unsafe {
            slice::from_raw_parts(
                self.ptr().add(mem::size_of::<DeviceNodeHeaderAbi>()),
                length,
            )
        }
    }
}

#[repr(C)]
pub struct DevicePathToTextAbi {
    device_node_to_text:
        unsafe extern "efiapi" fn(*const DeviceNodeHeaderAbi, bool, bool) -> *mut u16,
    device_path_to_text: unsafe extern "efiapi" fn(*const DevicePathAbi, bool, bool) -> *mut u16,
}

unsafe_protocol! {
    DevicePathToText(DevicePathToTextAbi, "8b843e20-8132-4852-90cc-551a4e4a7f1c");
}

impl DevicePathToText {
    pub fn device_node_to_text(
        &self,
        device_node: DeviceNode<'_>,
        display_only: bool,
        allow_shortcuts: bool,
    ) -> Result<NonNull<U16CStr>> {
        let p = unsafe {
            ((*self.abi()).device_node_to_text)(device_node.0, display_only, allow_shortcuts)
        };

        if p.is_null() {
            return Err(Status::OUT_OF_RESOURCES);
        }

        Ok(unsafe { NonNull::new_unchecked(U16CStr::from_ptr(p) as *const _ as *mut _) })
    }

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
