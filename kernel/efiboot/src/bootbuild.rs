use core::mem::{self, MaybeUninit};
use core::slice;

use uninit::extension_traits::AsOut;

use bootinfo::builder::Builder;
use bootinfo::item as bootitem;
use bootinfo::ItemKind;
use uefi::proto::gop::{self, GraphicsOutput};
use uefi::table::{BootServices, BootTable};
use uefi::{MemoryDescriptor, MemoryType, Result, Status};

use crate::page::{self, PAGE_SIZE};

const BOOTINFO_FIXED_SIZE: usize = 0x1000;
const MMAP_EXTRA_ENTRIES: usize = 8;

pub struct BootinfoCtx {
    pub efi_mmap_buf: &'static mut [MaybeUninit<u8>],
    pub mmap_scratch: &'static mut [MaybeUninit<bootitem::MemoryRange>],
    pub builder: Builder<'static>,
}

pub fn prepare_bootinfo(boot_table: &BootTable) -> Result<BootinfoCtx> {
    let boot_services = boot_table.boot_services();

    let (mmap_size, desc_size) = boot_services.memory_map_size()?;
    let max_mmap_entries = mmap_size / desc_size + MMAP_EXTRA_ENTRIES;

    let mut bootinfo_builder = make_bootinfo_builder(boot_services, max_mmap_entries)?;

    if let Ok(framebuffer) = get_framebuffer(boot_table) {
        append_bootinfo(&mut bootinfo_builder, ItemKind::FRAMEBUFFER, framebuffer)?;
    }

    append_bootinfo_slice(
        &mut bootinfo_builder,
        ItemKind::COMMAND_LINE,
        b"x86.serial=3f8 loglevel=debug",
    )?;

    Ok(BootinfoCtx {
        efi_mmap_buf: alloc_uninit_data(boot_services, max_mmap_entries * desc_size)?,
        mmap_scratch: alloc_uninit_data(boot_services, max_mmap_entries)?,
        builder: bootinfo_builder,
    })
}

pub fn append_mmap<'a>(
    builder: &mut Builder<'_>,
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
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA => bootitem::MemoryKind::USABLE,

        MemoryType::UNUSABLE => bootitem::MemoryKind::UNUSABLE,

        MemoryType::ACPI_RECLAIM => bootitem::MemoryKind::ACPI_TABLES,

        MemoryType::BOOT_SERVICES_DATA => bootitem::MemoryKind::FIRMWARE_BOOT,

        MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => {
            bootitem::MemoryKind::FIRMWARE_RUNIME
        }

        _ => bootitem::MemoryKind::RESERVED,
    }
}

fn get_framebuffer(boot_table: &BootTable) -> Result<bootitem::FramebufferInfo> {
    let current_mode = boot_table
        .boot_services()
        .locate_protocol::<GraphicsOutput>()?
        .current_mode();

    let mode_info = current_mode.info;

    let gop_framebuffer = current_mode.framebuffer.ok_or(Status::UNSUPPORTED)?;
    let format = match mode_info.pixel_format {
        gop::PixelFormat::Rgb => bootitem::PixelFormat::RGB,
        gop::PixelFormat::Bgr => bootitem::PixelFormat::BGR,
        _ => return Err(Status::UNSUPPORTED),
    };

    Ok(bootitem::FramebufferInfo {
        paddr: gop_framebuffer.base as usize,
        byte_size: gop_framebuffer.size,
        pixel_width: mode_info.hres,
        pixel_height: mode_info.vres,
        pixel_stride: mode_info.pixels_per_scanline,
        pixel_format: format,
    })
}

fn append_bootinfo<T>(builder: &mut Builder<'_>, kind: ItemKind, val: T) -> Result<()> {
    builder
        .append(kind, val)
        .map_err(|_| Status::OUT_OF_RESOURCES)
}

fn append_bootinfo_slice<T: Copy>(
    builder: &mut Builder<'_>,
    kind: ItemKind,
    val: &[T],
) -> Result<()> {
    builder
        .append_slice(kind, val)
        .map_err(|_| Status::OUT_OF_RESOURCES)
}

fn make_bootinfo_builder(
    boot_services: &BootServices,
    max_mmap_entries: usize,
) -> Result<Builder<'static>> {
    let buf = page::alloc_uninit_pages(
        boot_services,
        BOOTINFO_FIXED_SIZE + max_mmap_entries * mem::size_of::<bootitem::MemoryRange>(),
    )?;
    Ok(Builder::new(buf.as_out()).expect("buffer should be large and aligned"))
}

fn alloc_uninit_data<T>(
    boot_services: &BootServices,
    len: usize,
) -> Result<&'static mut [MaybeUninit<T>]> {
    let p = boot_services.alloc(len * mem::size_of::<T>())?;
    Ok(unsafe { slice::from_raw_parts_mut(p.cast(), len) })
}
