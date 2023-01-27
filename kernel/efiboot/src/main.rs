#![feature(alloc_error_handler, allocator_api, new_uninit)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use core::arch::asm;
use core::mem;
use core::panic::PanicInfo;

use alloc::boxed::Box;
use page::alloc_uninit_data;
use uefi::proto::fs::{File, OpenMode, SimpleFileSystem};
use uefi::proto::image::LoadedImage;
use uninit::extension_traits::AsOut;

use bootinfo::ItemKind;
use uefi::table::{BootServices, BootTable};
use uefi::{u16cstr, BootAlloc, Handle, Result, Status};

mod bootbuild;
mod elfload;
mod global_alloc;
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
fn handle_panic(_info: &PanicInfo<'_>) -> ! {
    halt()
}

#[no_mangle]
pub extern "efiapi" fn efi_main(image_handle: Handle, boot_table: BootTable) -> Status {
    run(image_handle, boot_table)
        .err()
        .unwrap_or(Status::LOAD_ERROR)
}

fn run(image_handle: Handle, boot_table: BootTable) -> Result<()> {
    let kernel_desc = load_kernel(image_handle, boot_table.boot_services())?;
    let bootinfo_ctx = bootbuild::prepare_bootinfo(kernel_desc.command_line, &boot_table)?;

    boot_table.exit_boot_services(
        image_handle,
        bootinfo_ctx.efi_mmap_buf.as_out(),
        move |runtime_table, mmap| {
            let mut builder = bootinfo_ctx.builder;
            builder
                .append(ItemKind::EFI_SYSTEM_TABLE, runtime_table)
                .unwrap();

            bootbuild::append_mmap(&mut builder, mmap, bootinfo_ctx.mmap_scratch);

            let bootinfo_slice = builder.finish();
            let entry: extern "sysv64" fn(usize, usize) -> ! =
                unsafe { mem::transmute(kernel_desc.kernel_entry) };

            entry(bootinfo_slice.as_ptr() as usize, bootinfo_slice.len());
        },
    )?;
}

struct KernelDesc {
    kernel_entry: u64,
    command_line: Option<&'static [u8]>,
}

fn load_kernel(image_handle: Handle, boot_services: &BootServices) -> Result<KernelDesc> {
    let loaded_image = boot_services.open_protocol::<LoadedImage>(image_handle, image_handle)?;

    let boot_fs = boot_services
        .open_protocol::<SimpleFileSystem>(loaded_image.device_handle(), image_handle)?;

    let root_dir = boot_fs.open_volume()?;
    let corrosios_dir = root_dir.open(u16cstr!("corrosios"), OpenMode::READ)?;

    let mut kernel_file = corrosios_dir.open(u16cstr!("kernel"), OpenMode::READ)?;
    let kernel_entry = elfload::load_elf(boot_services, &mut kernel_file)?;

    let command_line = load_command_line(&corrosios_dir, boot_services)?;

    Ok(KernelDesc {
        kernel_entry,
        command_line,
    })
}

fn load_command_line(
    corrosios_dir: &File<'_>,
    boot_services: &BootServices,
) -> Result<Option<&'static [u8]>> {
    let mut command_line_file = match corrosios_dir.open(u16cstr!("cmdline"), OpenMode::READ) {
        Ok(file) => file,
        Err(Status::NOT_FOUND) => return Ok(None),
        Err(e) => return Err(e),
    };

    let info_size = command_line_file.info_size()?;
    let mut info_buf = Box::new_uninit_slice_in(info_size, BootAlloc::new(boot_services));
    let info = command_line_file.info(info_buf.as_out())?;

    let command_line_size = info.size() as usize;
    let command_line = alloc_uninit_data(boot_services, command_line_size)?;

    let command_line = command_line_file.read_exact(command_line.as_out())?;
    Ok(Some(command_line))
}
