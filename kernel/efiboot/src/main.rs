#![feature(abi_efiapi)]
#![feature(alloc_error_handler, allocator_api)]
#![deny(unsafe_op_in_unsafe_fn)]
#![no_std]
#![no_main]

extern crate alloc;

use core::arch::asm;
use core::mem::{self, MaybeUninit};
use core::panic::PanicInfo;

use uninit::extension_traits::AsOut;

use bootinfo::builder::Builder;
use bootinfo::item as bootitem;
use bootinfo::ItemKind;
use uefi::table::BootTable;
use uefi::{Handle, MemoryDescriptor, MemoryType, Result, Status};

use page::PAGE_SIZE;

mod elfload;
mod global_alloc;
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
    let kernel_entry = setup::load_kernel(image_handle, boot_table.boot_services())?;
    let bootinfo_ctx = setup::prepare_bootinfo(&boot_table)?;

    let noret = boot_table.exit_boot_services(
        image_handle,
        bootinfo_ctx.efi_mmap_buf.as_out(),
        move |runtime_table, mmap| {
            let mut builder = bootinfo_ctx.builder;
            builder
                .append(ItemKind::EFI_SYSTEM_TABLE, runtime_table)
                .unwrap();

            append_mmap(&mut builder, mmap, bootinfo_ctx.mmap_scratch);

            let bootinfo_header = builder.finish();
            let entry: extern "sysv64" fn(usize) -> ! = unsafe { mem::transmute(kernel_entry) };

            entry(bootinfo_header as *const _ as usize);
        },
    )?;

    // One day we'll have `!` :(
    match noret {}
}

fn append_mmap<'a>(
    builder: &mut Builder,
    efi_mmap: impl ExactSizeIterator<Item = &'a MemoryDescriptor>,
    scratch: &mut [MaybeUninit<bootitem::MemoryRange>],
) {
    let tmp_mmap = scratch[..efi_mmap.len()]
        .as_out()
        .init_with(efi_mmap.map(|efi_desc| bootitem::MemoryRange {
            start_page: efi_desc.phys_start as usize / PAGE_SIZE,
            page_count: efi_desc.page_count as usize,
            kind: mem_kind_from_efi(efi_desc.mem_type),
        }));

    tmp_mmap.sort_unstable_by_key(|range| range.start_page);
    let tmp_mmap = coalesce_mmap(tmp_mmap);

    builder
        .append_slice(ItemKind::MEMORY_MAP, tmp_mmap)
        .unwrap();
}

fn coalesce_mmap(mmap: &mut [bootitem::MemoryRange]) -> &mut [bootitem::MemoryRange] {
    if mmap.is_empty() {
        return mmap;
    }

    let mut base = 0;

    for cur in 1..mmap.len() {
        let base_range = &mmap[base];
        let cur_range = &mmap[cur];

        let base_end = base_range.start_page + base_range.page_count;

        assert!(
            base_end <= cur_range.start_page,
            "intersecting memory map entries"
        );

        if base_range.kind == cur_range.kind && base_end == cur_range.start_page {
            // Entries can be merged, update our base entry in place and try to merge it with the
            // next entry.
            mmap[base].page_count += cur_range.page_count;
        } else {
            // Entries cannot be merged, move our base up and try again.
            base += 1;
            mmap[base] = *cur_range;
        }
    }

    &mut mmap[..base + 1]
}

fn mem_kind_from_efi(efi_type: MemoryType) -> bootitem::MemoryKind {
    match efi_type {
        MemoryType::CONVENTIONAL
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA => bootitem::MemoryKind::USABLE,

        MemoryType::UNUSABLE => bootitem::MemoryKind::UNUSABLE,

        MemoryType::ACPI_RECLAIM => bootitem::MemoryKind::ACPI_TABLES,

        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            bootitem::MemoryKind::FIRMWARE
        }

        _ => bootitem::MemoryKind::RESERVED,
    }
}
