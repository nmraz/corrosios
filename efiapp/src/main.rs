#![feature(abi_efiapi, asm)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec;
use core::fmt::Write;
use core::panic::PanicInfo;

use uefi::{BootTableHandle, Handle, Result, Status, MEMORY_TYPE_CONVENTIONAL};

mod allocator;

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

fn run(_image_handle: Handle, boot_table: BootTableHandle) -> Result<()> {
    allocator::with(&boot_table, || {
        let boot_services = boot_table.boot_services();
        let stdout = unsafe { &mut *boot_table.stdout() };

        stdout.reset()?;
        writeln!(
            stdout,
            "Firmware vendor: {}\nFirmware revision: {}\n",
            boot_table.firmware_vendor(),
            boot_table.firmware_revision()
        )
        .unwrap();

        let mmap_size = boot_services.memory_map_size()? + 0x100;
        let mut mmap_buf = vec![0u8; mmap_size];

        let (_key, mmap) = boot_services.memory_map(&mut mmap_buf)?;

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
    })
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: Handle, boot_table: BootTableHandle) -> Status {
    let _ = run(image_handle, boot_table);
    halt();
}
