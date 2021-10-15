#![feature(abi_efiapi, asm)]
#![feature(alloc_error_handler, allocator_api)]
#![no_std]
#![no_main]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

use core::fmt::Write;
use core::mem;
use core::panic::PanicInfo;

use uninit::extension_traits::AsOut;

use bootinfo::builder::Builder;
use bootinfo::{ItemKind, MemoryKind, MemoryRange};
use uefi::table::BootTable;
use uefi::{Handle, MemoryDescriptor, MemoryType, Result, Status};

use page::PAGE_SIZE;

mod allocator;
mod elfload;
mod page;
mod setup;

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
    run(image_handle, boot_table)
        .err()
        .unwrap_or(Status::LOAD_ERROR)
}

fn run(image_handle: Handle, boot_table: BootTable) -> Result<()> {
    let ctx = allocator::with(&boot_table, || setup::setup(image_handle, &boot_table))?;

    boot_table.stdout().reset()?;
    writeln!(
        boot_table.stdout(),
        "Kernel entry: {:#x}\nMemmap size: {:#x}",
        ctx.kernel_entry,
        ctx.mmap_buf.len()
    )
    .unwrap();

    let (runtime_table, mmap) =
        boot_table.exit_boot_services(image_handle, ctx.mmap_buf.as_out())?;

    let mut builder = ctx.bootinfo_builder;
    builder
        .append(ItemKind::EFI_SYSTEM_TABLE, runtime_table)
        .unwrap();

    append_mmap(&mut builder, mmap);

    let bootinfo_header = builder.finish();
    let entry: extern "sysv64" fn(usize) -> ! = unsafe { mem::transmute(ctx.kernel_entry) };

    entry(bootinfo_header as *const _ as usize);
}

fn append_mmap<'a>(
    builder: &mut Builder,
    mmap: impl ExactSizeIterator<Item = &'a MemoryDescriptor>,
) {
    // Safety: the loop below initializes all `mmap.len()` elements.
    let buf = unsafe { builder.reserve(ItemKind::MEMORY_MAP, mmap.len()) }.unwrap();

    for (efi_desc, range) in mmap.zip(buf) {
        range.write(MemoryRange {
            start_page: efi_desc.phys_start as usize / PAGE_SIZE,
            page_count: efi_desc.page_count as usize,
            kind: mem_kind_from_efi(efi_desc.mem_type),
        });
    }
}

fn mem_kind_from_efi(efi_type: MemoryType) -> MemoryKind {
    match efi_type {
        MemoryType::CONVENTIONAL
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA => MemoryKind::USABLE,

        MemoryType::UNUSABLE => MemoryKind::UNUSABLE,

        MemoryType::ACPI_RECLAIM => MemoryKind::ACPI_TABLES,

        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            MemoryKind::FIRMWARE
        }

        _ => MemoryKind::RESERVED,
    }
}
