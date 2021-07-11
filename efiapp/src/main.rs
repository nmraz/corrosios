#![feature(abi_efiapi, asm)]
#![no_std]
#![no_main]

use core::convert::TryFrom;
use core::fmt::{self, Write};
use core::panic::PanicInfo;
use core::{mem, slice};

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

const STATUS_SUCCESS: Status = 0;
const STATUS_WARN_UNKNOWN_GLYPH: Status = 1;

type Tpl = usize;
type Boolean = u8;
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

    raise_tpl: unsafe extern "efiapi" fn(Tpl) -> Tpl,
    restore_tpl: unsafe extern "efiapi" fn(Tpl),

    // TODO:
    allocate_pages: *const (),
    free_pages: *const (),
    get_memory_map: *const (),
    allocate_pool: *const (),
    free_pool: *const (),

    create_event: *const (),
    set_timer: *const (),
    wait_for_event: *const (),
    signal_event: *const (),
    close_event: *const (),
    check_event: *const (),
    // TODO...
}

#[repr(C)]
struct SimpleTextOutputProtocol {
    reset: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, Boolean) -> Status,
    output_string: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> Status,
    test_string: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, *const u16) -> Status,
    query_mode: *const (),
    set_mode: *const (),
    set_attribute: *const (),
    clear_screen: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol) -> Status,
    set_cursor_pos: *const (),
    enable_cursor: unsafe extern "efiapi" fn(*mut SimpleTextOutputProtocol, Boolean) -> Status,
    mode: *const (),
}

impl SimpleTextOutputProtocol {
    pub fn reset(&mut self) -> Status {
        unsafe { (self.reset)(self, 0) }
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
    let firmware_vendor = unsafe { CStr16::from_ptr(system_table.firmware_vendor) };
    let stdout = unsafe { &mut *system_table.console_out_protocol };

    stdout.reset();
    write!(
        stdout,
        "Firmware vendor: {}\nFirmware revision: {}",
        firmware_vendor, system_table.firmware_revision
    )
    .unwrap();

    halt();
}
