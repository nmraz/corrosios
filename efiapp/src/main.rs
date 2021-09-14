#![feature(abi_efiapi, asm)]
#![feature(alloc_error_handler, allocator_api)]
#![no_std]
#![no_main]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use core::fmt::Write;
use core::panic::PanicInfo;

use uefi::proto::image::LoadedImage;
use uefi::proto::io::SimpleTextOutput;
use uefi::proto::path::DevicePathToText;
use uefi::proto::ProtocolHandle;
use uefi::table::{BootServices, BootTableHandle, OpenProtocolHandle};
use uefi::{Handle, MemoryType, Result, Status};

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

fn run(image_handle: Handle, boot_table: &BootTableHandle) -> Result<()> {
    let boot_services = boot_table.boot_services();
    let mut stdout = boot_table.stdout();

    stdout.reset()?;
    writeln!(
        stdout,
        "Firmware vendor: {}\nFirmware revision: {}\n",
        boot_table.firmware_vendor(),
        boot_table.firmware_revision()
    )
    .unwrap();

    let loaded_image = boot_services.open_protocol::<LoadedImage>(image_handle, image_handle)?;

    print_image_info(boot_services, &mut stdout, &loaded_image)?;
    print_mem_map(boot_services, &mut stdout)?;

    Ok(())
}

fn print_image_info(
    boot_services: &BootServices,
    stdout: &mut ProtocolHandle<'_, SimpleTextOutput>,
    loaded_image: &OpenProtocolHandle<'_, LoadedImage>,
) -> Result<()> {
    writeln!(
        stdout,
        "Loaded image: base {:?}, size {}",
        loaded_image.image_base(),
        loaded_image.image_size(),
    )
    .unwrap();

    let device_path = loaded_image.file_path();
    let path_to_text = boot_services.locate_protocol::<DevicePathToText>()?;

    let path = {
        let raw = path_to_text.device_path_to_text(&device_path, true, true)?;
        unsafe { Box::from_raw(raw.as_ptr()) }
    };

    writeln!(stdout, "Image path: {}\n", path).unwrap();

    writeln!(stdout, "Path nodes:").unwrap();
    for device_node in device_path.nodes() {
        let node = unsafe {
            Box::from_raw(
                path_to_text
                    .device_node_to_text(device_node, true, true)?
                    .as_ptr(),
            )
        };

        writeln!(stdout, "{}", node).unwrap();
    }
    writeln!(stdout).unwrap();

    Ok(())
}

fn print_mem_map(
    boot_services: &BootServices,
    stdout: &mut ProtocolHandle<'_, SimpleTextOutput>,
) -> Result<()> {
    let mmap_size = boot_services.memory_map_size()? + 0x100;
    let mut mmap_buf = vec![0u8; mmap_size];

    let (_key, mmap) = boot_services.memory_map(&mut mmap_buf)?;

    let conventional_mem_pages: u64 = mmap
        .filter(|desc| desc.mem_type == MemoryType::CONVENTIONAL)
        .map(|desc| desc.page_count)
        .sum();

    writeln!(
        stdout,
        "Free memory: {} pages (~{}MB)",
        conventional_mem_pages,
        (conventional_mem_pages * 0x1000) / 0x100000
    )
    .unwrap();

    Ok(())
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: Handle, boot_table: BootTableHandle) -> Status {
    let res = allocator::with(&boot_table, || run(image_handle, &boot_table));

    if let Err(status) = res {
        writeln!(boot_table.stdout(), "Error: {:#x}", status.0).unwrap();
    }

    halt();
}
