#![feature(abi_efiapi, asm)]
#![no_std]
#![no_main]

use core::convert::TryFrom;
use core::fmt::{self, Write};
use core::mem::MaybeUninit;
use core::panic::PanicInfo;
use core::{mem, ptr, slice};

fn halt() -> ! {
    unsafe {
        asm!("cli");
        loop {
            asm!("hlt");
        }
    }
}

#[panic_handler]
fn handle_panic(_info: &PanicInfo) -> ! {
    halt()
}

type Status = usize;

const ERROR_BIT: Status = 1 << (mem::size_of::<usize>() * 8 - 1);

const STATUS_SUCCESS: Status = 0;
const STATUS_BUFFER_TOO_SMALL: Status = 5 | ERROR_BIT;
const STATUS_WARN_UNKNOWN_GLYPH: Status = 1;

type MemoryType = u32;

const MEMORY_TYPE_RESERVED: MemoryType = 0;
const MEMORY_TYPE_LOADER_CODE: MemoryType = 1;
const MEMORY_TYPE_LOADER_DATA: MemoryType = 2;
const MEMORY_TYPE_BOOT_SERVICES_CODE: MemoryType = 3;
const MEMORY_TYPE_BOOT_SERVICES_DATA: MemoryType = 4;
const MEMORY_TYPE_RUNTIME_SERVICES_CODE: MemoryType = 5;
const MEMORY_TYPE_RUNTIME_SERVICES_DATA: MemoryType = 6;
const MEMORY_TYPE_CONVENTIONAL: MemoryType = 7;
const MEMORY_TYPE_UNUSABLE: MemoryType = 8;

type Handle = *mut ();

#[repr(transparent)]
struct CStr16([u16]);

impl CStr16 {
    pub unsafe fn from_ptr<'a>(ptr: *const u16) -> &'a Self {
        let mut len = 0;
        while *ptr.add(len) != 0 {
            len += 1;
        }

        let data = slice::from_raw_parts(ptr, len);
        mem::transmute(data)
    }

    pub fn as_slice(&self) -> &[u16] {
        // SAFETY: transparent representation
        unsafe { mem::transmute(self) }
    }
}

impl fmt::Display for CStr16 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &c in self.as_slice() {
            char::try_from(c as u32).map_err(|_| fmt::Error)?.fmt(f)?;
        }
        Ok(())
    }
}

#[repr(C)]
pub struct MemoryDescriptor {
    mem_type: MemoryType,
    phys_start: u64,
    virt_start: u64,
    page_count: u64,
    attr: u64,
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
        *mut usize,
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
    // TODO...
}

impl BootServices {
    pub fn memory_map_size(&self) -> usize {
        let mut mmap_size = 0;
        let mut key = 0;
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
        assert_eq!(status, STATUS_BUFFER_TOO_SMALL);

        mmap_size
    }

    /// # Safety
    /// Alignment
    pub unsafe fn memory_map<'a>(
        &self,
        buf: &'a mut [MaybeUninit<u8>],
    ) -> impl Iterator<Item = &'a MemoryDescriptor> {
        let mut size = buf.len();
        let mut key = 0;
        let mut desc_size = 0;
        let mut version = 0;

        let status = (self.get_memory_map)(
            &mut size,
            &mut buf[0] as *mut _ as *mut MemoryDescriptor,
            &mut key,
            &mut desc_size,
            &mut version,
        );
        assert_eq!(status, STATUS_SUCCESS);

        buf[..size].chunks(desc_size).map(move |chunk| {
            assert_eq!(chunk.len(), desc_size);
            // SAFETY: aligned, we trust the firmware
            unsafe { &*(&chunk[0] as *const _ as *const MemoryDescriptor) }
        })
    }

    pub fn alloc(&self, size: usize) -> *mut u8 {
        let mut p = ptr::null_mut();
        let status = unsafe { (self.allocate_pool)(MEMORY_TYPE_LOADER_DATA, size, &mut p) };
        assert_eq!(status, STATUS_SUCCESS);

        p
    }

    /// # Safety
    /// TODO
    pub unsafe fn free(&self, p: *mut u8) {
        (self.free_pool)(p);
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
    pub fn reset(&mut self) -> Status {
        unsafe { (self.reset)(self, false) }
    }

    pub fn output_string(&mut self, string: &str) -> Status {
        const BUF_LEN: usize = 64;

        let mut buf = [0u16; BUF_LEN + 1];
        let mut i = 0;

        let mut status = STATUS_SUCCESS;

        let mut putchar = |ch| {
            if i == BUF_LEN {
                status = unsafe { (self.output_string)(self, &buf[0]) };

                buf.fill(0);
                i = 0;

                if status != STATUS_SUCCESS {
                    return Err(ucs2::Error::MultiByte);
                }
            }

            buf[i] = ch;
            i += 1;

            Ok(())
        };

        let res = ucs2::encode_with(string, |ch| {
            if ch == b'\n' as u16 {
                putchar(b'\r' as u16)?;
            }
            putchar(ch)
        });

        if res.is_err() {
            return if status == STATUS_SUCCESS {
                STATUS_WARN_UNKNOWN_GLYPH
            } else {
                status
            };
        }

        unsafe { (self.output_string)(self, &buf[0]) }
    }
}

impl fmt::Write for SimpleTextOutputProtocol {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let status = self.output_string(s);
        if status != STATUS_SUCCESS {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

#[repr(C)]
pub struct SystemTable {
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

#[no_mangle]
pub extern "efiapi" fn efi_main(
    _image_handle: Handle,
    system_table: &'static SystemTable,
) -> Status {
    let boot_services = unsafe { &*system_table.boot_services };
    let stdout = unsafe { &mut *system_table.console_out_protocol };

    let firmware_vendor = unsafe { CStr16::from_ptr(system_table.firmware_vendor) };

    stdout.reset();
    writeln!(
        stdout,
        "Firmware vendor: {}\nFirmware revision: {}\n",
        firmware_vendor, system_table.firmware_revision
    )
    .unwrap();

    let mmap_size = boot_services.memory_map_size() + 0x100;
    let mmap_buf = {
        let buf = boot_services.alloc(mmap_size) as *mut _;
        unsafe { slice::from_raw_parts_mut(buf, mmap_size) }
    };

    let mmap = unsafe { boot_services.memory_map(mmap_buf) };

    let conventional_mem_pages: u64 = mmap
        .filter(|desc| desc.mem_type == MEMORY_TYPE_CONVENTIONAL)
        .map(|desc| desc.page_count)
        .sum();

    writeln!(stdout, "Free memory: {} pages", conventional_mem_pages).unwrap();

    halt();
}
