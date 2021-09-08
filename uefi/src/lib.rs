#![feature(abi_efiapi, asm)]
#![no_std]

use core::convert::TryFrom;
use core::marker::PhantomData;
use core::{fmt, mem, ptr, result, slice};

pub use status::{Result, Status};

mod status;

#[repr(transparent)]
pub struct Handle(*const ());

#[repr(transparent)]
pub struct U16CStr([u16]);

impl U16CStr {
    /// # Safety
    ///
    /// Must be null-terminated.
    pub unsafe fn from_ptr<'a>(ptr: *const u16) -> &'a Self {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let data = slice::from_raw_parts(ptr, len);
        mem::transmute(data)
    }

    pub fn as_slice(&self) -> &[u16] {
        &self.0
    }

    pub fn as_ptr(&self) -> *const u16 {
        self.as_slice().as_ptr()
    }
}

impl fmt::Display for U16CStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.as_slice() {
            char::try_from(c as u32).map_err(|_| fmt::Error)?.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryMapKey(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct MemoryType(pub u32);

impl MemoryType {
    pub const RESERVED: Self = Self(0);
    pub const LOADER_CODE: Self = Self(1);
    pub const LOADER_DATA: Self = Self(2);
    pub const BOOT_SERVICES_CODE: Self = Self(3);
    pub const BOOT_SERVICES_DATA: Self = Self(4);
    pub const RUNTIME_SERVICES_CODE: Self = Self(5);
    pub const RUNTIME_SERVICES_DATA: Self = Self(6);
    pub const CONVENTIONAL: Self = Self(7);
    pub const UNUSABLE: Self = Self(8);
}

#[repr(C)]
pub struct MemoryDescriptor {
    pub mem_type: MemoryType,
    pub phys_start: u64,
    pub virt_start: u64,
    pub page_count: u64,
    pub attr: u64,
}

#[repr(C)]
pub struct TableHeader {
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
pub struct SimpleTextOutputProtocol {
    reset: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, bool) -> Status,
    output_string: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> Status,
    test_string: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> Status,
    query_mode: *const (),
    set_mode: *const (),
    set_attribute: *const (),
    clear_screen: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol) -> Status,
    set_cursor_pos: *const (),
    enable_cursor: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, bool) -> Status,
    mode: *const (),
}

impl SimpleTextOutputProtocol {
    pub fn reset(&mut self) -> Result<()> {
        unsafe { (self.reset)(self, false) }.to_result()
    }

    /// # Safety
    ///
    /// Must be null-terminated.
    pub unsafe fn output_string_unchecked(&mut self, s: *const u16) -> Result<()> {
        (self.output_string)(self, s).to_result()
    }

    pub fn output_u16_str(&mut self, s: &U16CStr) -> Result<()> {
        unsafe { self.output_string_unchecked(s.as_ptr()) }
    }

    pub fn output_str(&mut self, s: &str) -> Result<()> {
        const BUF_LEN: usize = 64;

        let mut buf = [0u16; BUF_LEN + 1];
        let mut i = 0;

        let mut status = Ok(());

        let mut putchar = |ch| {
            if i == BUF_LEN {
                status = unsafe { self.output_string_unchecked(buf.as_ptr()) };
                status.map_err(|_| ucs2::Error::BufferOverflow)?;

                buf.fill(0);
                i = 0;
            }

            buf[i] = ch;
            i += 1;

            Ok(())
        };

        let res = ucs2::encode_with(s, |ch| {
            if ch == b'\n' as u16 {
                putchar(b'\r' as u16)?;
            }
            putchar(ch)
        });

        status?;
        res.map_err(|_| Status::WARN_UNKNOWN_GLYPH)?;

        unsafe { self.output_string_unchecked(buf.as_ptr()) }
    }
}

impl fmt::Write for SimpleTextOutputProtocol {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.output_str(s).map_err(|_| fmt::Error)
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
    console_out_protocol: *mut SimpleTextOutputProtocol,
    stderr_handle: Handle,
    stderr_protocol: *mut SimpleTextOutputProtocol,
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

    pub fn stdout(&self) -> *mut SimpleTextOutputProtocol {
        self.0.console_out_protocol
    }
}
