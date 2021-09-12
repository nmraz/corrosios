use core::marker::PhantomData;
use core::{mem, ptr, result};

use crate::proto::io::{SimpleTextOutput, SimpleTextOutputAbi};
use crate::proto::ProtocolHandle;
use crate::types::{Handle, MemoryDescriptor, MemoryMapKey, MemoryType, U16CStr};
use crate::{Result, Status};

#[repr(C)]
struct TableHeader {
    signature: u64,
    revision: u32,
    header_size: u32,
    crc32: u32,
    reserved: u32,
}

#[repr(C)]
pub struct BootServices {
    header: TableHeader,

    raise_tpl: *const (),
    restore_tpl: *const (),

    // TODO:
    allocate_pages: *const (),
    free_pages: *const (),
    get_memory_map: unsafe extern "efiapi" fn(
        *mut usize,
        *mut MemoryDescriptor,
        *mut MemoryMapKey,
        *mut usize,
        *mut u32,
    ) -> Status,
    allocate_pool: unsafe extern "efiapi" fn(MemoryType, usize, *mut *mut u8) -> Status,
    free_pool: unsafe extern "efiapi" fn(*mut u8) -> Status,

    create_event: *const (),
    set_timer: *const (),
    wait_for_event: *const (),
    signal_event: *const (),
    close_event: *const (),
    check_event: *const (),

    install_protocol_interface: *const (),
    reinstall_protocol_interface: *const (),
    uninstall_protocol_interface: *const (),
    handle_protocol: *const (),
    reserved: *const (),
    register_protocol_notify: *const (),
    locate_handle: *const (),
    locate_device_path: *const (),
    install_configuration_table: *const (),

    load_image: *const (),
    start_image: *const (),
    exit: *const (),
    unload_image: *const (),
    exit_boot_services: unsafe extern "efiapi" fn(Handle, MemoryMapKey) -> Status,
    // TODO...
}

impl BootServices {
    pub fn memory_map_size(&self) -> Result<usize> {
        let mut mmap_size = 0;
        let mut key = MemoryMapKey(0);
        let mut desc_size = 0;
        let mut version = 0;

        let status = unsafe {
            (self.get_memory_map)(
                &mut mmap_size,
                ptr::null_mut(),
                &mut key,
                &mut desc_size,
                &mut version,
            )
        };

        if status != Status::BUFFER_TOO_SMALL {
            status.to_result()?;
        }

        Ok(mmap_size)
    }

    pub fn memory_map<'a>(
        &self,
        buf: &'a mut [u8],
    ) -> Result<(
        MemoryMapKey,
        impl ExactSizeIterator<Item = &'a MemoryDescriptor> + Clone,
    )> {
        let mut size = buf.len();
        let mut key = MemoryMapKey(0);
        let mut desc_size = 0;
        let mut version = 0;

        assert_eq!(
            buf.as_ptr() as usize % mem::align_of::<MemoryDescriptor>(),
            0
        );

        // Safety: buffer is suitably aligned.
        unsafe {
            (self.get_memory_map)(
                &mut size,
                buf.as_mut_ptr() as *mut MemoryDescriptor,
                &mut key,
                &mut desc_size,
                &mut version,
            )
        }
        .to_result()?;

        let iter = buf[..size].chunks(desc_size).map(move |chunk| {
            assert_eq!(chunk.len(), desc_size);
            // Safety: aligned, we trust the firmware
            unsafe { &*(chunk.as_ptr() as *const MemoryDescriptor) }
        });

        Ok((key, iter))
    }

    pub fn alloc(&self, size: usize) -> Result<*mut u8> {
        let mut p = ptr::null_mut();
        unsafe { (self.allocate_pool)(MemoryType::LOADER_DATA, size, &mut p) }.to_result()?;

        assert_ne!(p, ptr::null_mut());
        Ok(p)
    }

    /// # Safety
    ///
    /// Must have been allocated with `alloc`.
    pub unsafe fn free(&self, p: *mut u8) {
        (self.free_pool)(p).to_result().expect("invalid pointer");
    }
}

#[repr(C)]
struct SystemTable {
    header: TableHeader,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    console_in_handle: Handle,
    console_in_protocol: Handle, // TODO
    console_out_handle: Handle,
    console_out_protocol: *mut SimpleTextOutputAbi,
    stderr_handle: Handle,
    stderr_protocol: *mut SimpleTextOutputAbi,
    runtime_services: *const (), // TODO
    boot_services: *const BootServices,
    num_entries: usize,
    configuration_table: Handle, // TODO
}

pub trait TableState {}

pub struct BootState;
impl TableState for BootState {}

pub struct RuntimeState;
impl TableState for RuntimeState {}

#[repr(transparent)]
pub struct SystemTableHandle<S: TableState>(&'static SystemTable, PhantomData<S>);

pub type BootTableHandle = SystemTableHandle<BootState>;
pub type RuntimeTableHandle = SystemTableHandle<RuntimeState>;

impl<S: TableState> SystemTableHandle<S> {
    fn new(ptr: &'static SystemTable) -> Self {
        Self(ptr, PhantomData)
    }

    pub fn firmware_vendor(&self) -> &U16CStr {
        unsafe { U16CStr::from_ptr(self.0.firmware_vendor) }
    }

    pub fn firmware_revision(&self) -> u32 {
        self.0.firmware_revision
    }
}

pub enum ExitBootServicesError {
    StaleMemoryMap(BootTableHandle),
    Error(Status),
}

impl BootTableHandle {
    pub fn boot_services(&self) -> &BootServices {
        // Safety: we haven't exited boot services, so this pointer is valid.
        unsafe { &*self.0.boot_services }
    }

    /// # Safety
    ///
    /// If this function fails due to a stale memory map (and returns the existing table handle),
    /// the only boot services that can be called are those related to memory allocation.
    pub unsafe fn exit_boot_services(
        self,
        image_handle: Handle,
        key: MemoryMapKey,
    ) -> result::Result<RuntimeTableHandle, ExitBootServicesError> {
        let ptr = self.0;
        (self.boot_services().exit_boot_services)(image_handle, key)
            .to_result()
            .map_err(|status| {
                if status == Status::INVALID_PARAMETER {
                    ExitBootServicesError::StaleMemoryMap(BootTableHandle::new(ptr))
                } else {
                    ExitBootServicesError::Error(status)
                }
            })?;

        Ok(RuntimeTableHandle::new(ptr))
    }

    pub fn stdout(&self) -> ProtocolHandle<'_, SimpleTextOutput> {
        unsafe { ProtocolHandle::from_abi(self.0.console_out_protocol) }
    }
}
