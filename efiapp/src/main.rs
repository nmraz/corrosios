#![feature(abi_efiapi, asm)]
#![no_std]
#![no_main]

use core::fmt::Write;
use core::panic::PanicInfo;

use arrayvec::ArrayString;

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

type Tpl = usize;
type Boolean = u8;
type Handle = *mut ();

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
    runtime_services: Handle, // TODO
    boot_services: *const BootServices,
    num_entries: usize,
    configuration_table: Handle, // TODO
}

#[no_mangle]
pub extern "efiapi" fn efi_main(_image_handle: Handle, system_table: *const SystemTable) -> Status {
    let system_table = unsafe { &*system_table };
    let console_out = unsafe { &mut *system_table.console_out_protocol };

    let mut msg = ArrayString::<30>::new();
    let mut str_buf = [0u16; 30];

    write!(
        &mut msg,
        "Firmware revision: {}",
        system_table.firmware_revision
    )
    .unwrap();

    for (cp, pos) in msg.encode_utf16().zip(&mut str_buf) {
        *pos = cp as u16;
    }

    unsafe {
        (console_out.reset)(console_out, 0);
        (console_out.output_string)(console_out, &str_buf[0]);
    }

    halt();
}
