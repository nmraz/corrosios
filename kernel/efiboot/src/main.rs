#![feature(abi_efiapi)]
#![feature(alloc_error_handler, allocator_api)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use core::arch::asm;
use core::mem;
use core::panic::PanicInfo;

use uefi::proto::fs::{OpenMode, SimpleFileSystem};
use uefi::proto::image::LoadedImage;
use uninit::extension_traits::AsOut;

use bootinfo::ItemKind;
use uefi::table::{BootServices, BootTable};
use uefi::{u16cstr, Handle, Result, Status};

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
    let kernel_entry = load_kernel(image_handle, boot_table.boot_services())?;
    let bootinfo_ctx = bootbuild::prepare_bootinfo(&boot_table)?;

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
                unsafe { mem::transmute(kernel_entry) };

            entry(bootinfo_slice.as_ptr() as usize, bootinfo_slice.len());
        },
    )?;
}

fn load_kernel(image_handle: Handle, boot_services: &BootServices) -> Result<u64> {
    let loaded_image = boot_services.open_protocol::<LoadedImage>(image_handle, image_handle)?;

    let boot_fs = boot_services
        .open_protocol::<SimpleFileSystem>(loaded_image.device_handle(), image_handle)?;

    let root_dir = boot_fs.open_volume()?;
    let mut file = root_dir.open(u16cstr!("corrosios\\kernel"), OpenMode::READ)?;

    elfload::load_elf(boot_services, &mut file)
}
