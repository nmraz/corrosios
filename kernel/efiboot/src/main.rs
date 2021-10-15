#![feature(abi_efiapi, asm)]
#![feature(alloc_error_handler, allocator_api)]
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use core::fmt::Write;
use core::panic::PanicInfo;

use uefi::proto::fs::{OpenMode, SimpleFileSystem};
use uefi::proto::image::LoadedImage;
use uefi::proto::io::SimpleTextOutput;
use uefi::proto::path::DevicePathToText;
use uefi::proto::ProtocolHandle;
use uefi::table::{BootServices, BootTable, OpenProtocolHandle};
use uefi::{u16cstr, Handle, MemoryType, Result, Status};

use uninit::extension_traits::AsOut;

mod allocator;
mod elfload;
mod page;

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

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: Handle, boot_table: BootTable) -> Status {
    let res = allocator::with(&boot_table, || run(image_handle, &boot_table));

    if let Err(status) = res {
        writeln!(boot_table.stdout(), "Error: {:#x}", status.0).unwrap();
    }

    halt();
}

fn run(image_handle: Handle, boot_table: &BootTable) -> Result<()> {
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

    print_bootloader_info(boot_services, &mut stdout, &loaded_image)?;

    let kentry = load_kernel(boot_services, image_handle, &loaded_image)?;
    writeln!(stdout, "Loaded kernel, entry point: {:#x}\n", kentry).unwrap();

    print_mem_map(boot_services, &mut stdout)?;

    Ok(())
}

fn print_bootloader_info(
    boot_services: &BootServices,
    stdout: &mut ProtocolHandle<'_, SimpleTextOutput>,
    loaded_image: &OpenProtocolHandle<'_, LoadedImage>,
) -> Result<()> {
    let device_path = loaded_image.file_path();
    let path_to_text = boot_services.locate_protocol::<DevicePathToText>()?;

    let path = {
        let raw = path_to_text.device_path_to_text(&device_path, true, true)?;
        unsafe { Box::from_raw(raw.as_ptr()) }
    };

    writeln!(stdout, "Bootloader path: {}\n", path).unwrap();

    Ok(())
}

fn load_kernel(
    boot_services: &BootServices,
    image_handle: Handle,
    loaded_image: &OpenProtocolHandle<'_, LoadedImage>,
) -> Result<u64> {
    let boot_fs = boot_services
        .open_protocol::<SimpleFileSystem>(loaded_image.device_handle(), image_handle)?;

    let root_dir = boot_fs.open_volume()?;
    let mut file = root_dir.open(u16cstr!("regasos\\kernel"), OpenMode::READ)?;

    elfload::load_elf(boot_services, &mut file)
}

fn print_mem_map(
    boot_services: &BootServices,
    stdout: &mut ProtocolHandle<'_, SimpleTextOutput>,
) -> Result<()> {
    let mmap_size = boot_services.memory_map_size()? + 0x100;
    let mut mmap_buf = vec![0; mmap_size];

    let (_key, mmap) = boot_services.memory_map(mmap_buf.as_out())?;

    let conventional_mem_pages: u64 = mmap
        .clone()
        .filter(|desc| desc.mem_type == MemoryType::CONVENTIONAL)
        .map(|desc| desc.page_count)
        .sum();

    writeln!(
        stdout,
        "Free memory: {} pages (~{}MB)\n",
        conventional_mem_pages,
        (conventional_mem_pages * 0x1000) / 0x100000
    )
    .unwrap();

    let low_mem = mmap.filter(|desc| desc.phys_start < 0x1000000);

    writeln!(stdout, "Low memory map:").unwrap();
    for desc in low_mem {
        let mem_type = match desc.mem_type {
            MemoryType::BOOT_SERVICES_CODE => "boot services code",
            MemoryType::BOOT_SERVICES_DATA => "boot services data",
            MemoryType::RUNTIME_SERVICES_CODE => "runtime services code",
            MemoryType::RUNTIME_SERVICES_DATA => "runtime services data",
            MemoryType::LOADER_CODE => "loader code",
            MemoryType::LOADER_DATA => "loader data",
            MemoryType::CONVENTIONAL => "usable",
            MemoryType::RESERVED => "reserved",
            MemoryType::UNUSABLE => "unusable",
            _ => "other",
        };

        writeln!(
            stdout,
            "{:#x}-{:#x}: {}",
            desc.phys_start,
            desc.phys_start + desc.page_count * 0x1000,
            mem_type
        )
        .unwrap();
    }

    Ok(())
}
