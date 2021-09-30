use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};
use core::{mem, ptr, result};

use crate::proto::io::{SimpleTextOutput, SimpleTextOutputAbi};
use crate::proto::{Protocol, ProtocolHandle};
use crate::{Guid, Handle, MemoryDescriptor, MemoryMapKey, MemoryType, Result, Status, U16CStr};

pub struct OpenProtocolHandle<'a, P: Protocol> {
    proto: P,
    handle: Handle,
    boot_services: &'a BootServices,
    image_handle: Handle,
}

impl<'a, P: Protocol> OpenProtocolHandle<'a, P> {
    /// # Safety
    ///
    /// ABI pointer must be valid and outlive `'a`. The protocol must have been opened via a call
    /// to `open_protocol` on `handle`, with `image_handle` passed as the agent.
    unsafe fn from_abi(
        abi: *mut P::Abi,
        handle: Handle,
        boot_services: &'a BootServices,
        image_handle: Handle,
    ) -> Self {
        Self {
            proto: P::from_abi(abi),
            handle,
            boot_services,
            image_handle,
        }
    }
}

impl<P: Protocol> Deref for OpenProtocolHandle<'_, P> {
    type Target = P;

    fn deref(&self) -> &P {
        &self.proto
    }
}

impl<P: Protocol> DerefMut for OpenProtocolHandle<'_, P> {
    fn deref_mut(&mut self) -> &mut P {
        &mut self.proto
    }
}

impl<P: Protocol> Drop for OpenProtocolHandle<'_, P> {
    fn drop(&mut self) {
        unsafe {
            (self.boot_services.close_protocol)(
                self.handle,
                &P::GUID,
                self.image_handle,
                Handle(ptr::null()),
            )
        }
        .to_result()
        .expect("failed to close existing protocol handle");
    }
}

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

    get_next_monotonic_count: *const (),
    stall: *const (),
    set_watchdog_timer: *const (),

    connect_controller: *const (),
    disconnect_controller: *const (),

    open_protocol:
        unsafe extern "efiapi" fn(Handle, *const Guid, *mut *mut u8, Handle, Handle, u32) -> Status,
    close_protocol: unsafe extern "efiapi" fn(Handle, *const Guid, Handle, Handle) -> Status,
    open_protocol_information: *const (),

    protocols_per_handle: *const (),
    locate_handle_buffer: *const (),
    locate_protocol: unsafe extern "efiapi" fn(*const Guid, *const u8, *mut *mut u8) -> Status,
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

    pub fn open_protocol<P: Protocol>(
        &self,
        handle: Handle,
        image_handle: Handle,
    ) -> Result<OpenProtocolHandle<'_, P>> {
        const OPEN_BY_HANDLE_PROTOCOL: u32 = 1;

        let mut abi = ptr::null_mut();

        unsafe {
            (self.open_protocol)(
                handle,
                &P::GUID,
                &mut abi as *mut _ as *mut *mut _,
                image_handle,
                Handle(ptr::null()),
                OPEN_BY_HANDLE_PROTOCOL,
            )
        }
        .to_result()?;

        Ok(unsafe { OpenProtocolHandle::from_abi(abi, handle, self, image_handle) })
    }

    pub fn locate_protocol<P: Protocol>(&self) -> Result<ProtocolHandle<'_, P>> {
        let mut abi = ptr::null_mut();

        unsafe { (self.locate_protocol)(&P::GUID, ptr::null(), &mut abi as *mut _ as *mut *mut _) }
            .to_result()?;

        // Safety: if we get here, the pointer is guaranteed to be valid and of the correct type
        // as `P::GUID` can be trusted. The protocol instance lives at least as long as `self`,
        // meaning that the lifetime is correct as well.
        Ok(unsafe { ProtocolHandle::from_abi(abi) })
    }
}

#[repr(C)]
struct SystemTable {
    header: TableHeader,
    firmware_vendor: *const u16,
    firmware_revision: u32,
    console_in_handle: Handle,
    console_in_protocol: *const (), // TODO
    console_out_handle: Handle,
    console_out_protocol: *mut SimpleTextOutputAbi,
    stderr_handle: Handle,
    stderr_protocol: *mut SimpleTextOutputAbi,
    runtime_services: *const (), // TODO
    boot_services: *const BootServices,
    num_entries: usize,
    configuration_table: *const (), // TODO
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
