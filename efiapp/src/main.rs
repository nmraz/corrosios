#![feature(abi_efiapi, asm)]
#![no_std]
#![no_main]

use core::fmt::Write;
use core::panic::PanicInfo;
use core::slice;

use uefi::{BootState, Handle, Result, Status, SystemTableHandle, MEMORY_TYPE_CONVENTIONAL};

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

fn run(_image_handle: Handle, system_table: SystemTableHandle<BootState>) -> Result<()> {
    let boot_services = system_table.boot_services();
    let stdout = unsafe { &mut *system_table.stdout() };

    stdout.reset()?;
    writeln!(
        stdout,
        "Firmware vendor: {}\nFirmware revision: {}\n",
        system_table.firmware_vendor(),
        system_table.firmware_revision()
    )
    .unwrap();

    let mmap_size = boot_services.memory_map_size()? + 0x100;
    let mmap_buf = {
        let buf = boot_services.alloc(mmap_size)?;
        unsafe { slice::from_raw_parts_mut(buf, mmap_size) }
    };

    let (_key, mmap) = boot_services.memory_map(mmap_buf)?;

    let conventional_mem_pages: u64 = mmap
        .filter(|desc| desc.mem_type == MEMORY_TYPE_CONVENTIONAL)
        .map(|desc| desc.page_count)
        .sum();

    writeln!(
        stdout,
        "Free memory: {} pages (~{} MiB)",
        conventional_mem_pages,
        (conventional_mem_pages * 0x1000) / 0x100000
    )
    .unwrap();

    Ok(())
}

#[no_mangle]
pub extern "efiapi" fn efi_main(
    image_handle: Handle,
    system_table: SystemTableHandle<BootState>,
) -> Status {
    let _ = run(image_handle, system_table);
    halt();
}
