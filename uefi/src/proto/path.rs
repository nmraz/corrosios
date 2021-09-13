use core::marker::PhantomData;
use core::ptr::{self, NonNull};
use core::{iter, mem, slice};

use crate::types::U16CStr;
use crate::{Result, Status};

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
    DevicePath(DevicePathAbi,
        (0x09576e91, 0x6d3f, 0x11d2, [0x8e, 0x39, 0x00, 0xa0, 0xc9, 0x69, 0x72, 0x3b]));
}

impl DevicePath {
    pub fn nodes(&self) -> impl Iterator<Item = DeviceNode<'_>> + Clone {
        let init = unsafe { DeviceNode::new(self.abi() as *const DeviceNodeHeaderAbi) };

        iter::successors(Some(init), |cur| {
            if cur.node_type() == DeviceNode::TYPE_END
                && cur.sub_type() == DeviceNode::SUB_TYPE_END_ENTIRE
            {
                None
            } else {
                Some(unsafe { DeviceNode::new(cur.ptr.add(cur.header().length as usize)) })
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DeviceNode<'a> {
    ptr: *const DeviceNodeHeaderAbi,
    marker: PhantomData<&'a DevicePath>,
}

impl<'a> DeviceNode<'a> {
    pub const TYPE_HARDWARE: u8 = 0x1;
    pub const TYPE_ACPI: u8 = 0x2;
    pub const TYPE_MESSAGING: u8 = 0x3;
    pub const TYPE_MEDIA: u8 = 0x4;
    pub const TYPE_BIOS: u8 = 0x5;
    pub const TYPE_END: u8 = 0x7f;

    pub const SUB_TYPE_END_ENTIRE: u8 = 0xff;
    pub const SUB_TYPE_END_DEVICE: u8 = 0x1;

    /// # Safety
    ///
    /// ABI pointer must be valid and live long enough.
    unsafe fn new(abi: *const DeviceNodeHeaderAbi) -> Self {
        Self {
            ptr: abi,
            marker: PhantomData,
        }
    }

    fn header(&self) -> DeviceNodeHeaderAbi {
        unsafe { ptr::read_unaligned(self.ptr) }
    }

    pub fn node_type(&self) -> u8 {
        self.header().node_type
    }

    pub fn sub_type(&self) -> u8 {
        self.header().sub_type
    }

    pub fn data(&self) -> &'a [u8] {
        let full_length = self.header().length as usize;
        assert!(full_length >= mem::size_of::<DeviceNodeHeaderAbi>());

        let length = full_length - mem::size_of::<DeviceNodeHeaderAbi>();
        unsafe {
            slice::from_raw_parts(
                self.ptr
                    .cast::<u8>()
                    .add(mem::size_of::<DeviceNodeHeaderAbi>()),
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
    DevicePathToText(DevicePathToTextAbi,
        (0x8b843e20, 0x8132, 0x4852, [0x90, 0xcc, 0x55, 0x1a, 0x4e, 0x4a, 0x7f, 0x1c]));
}

impl DevicePathToText {
    pub fn device_node_to_text(
        &self,
        device_node: DeviceNode<'_>,
        display_only: bool,
        allow_shortcuts: bool,
    ) -> Result<NonNull<U16CStr>> {
        let p = unsafe {
            ((*self.abi()).device_node_to_text)(device_node.ptr, display_only, allow_shortcuts)
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
