use core::mem::{self, MaybeUninit};
use core::slice;

use uninit::extension_traits::AsOut;

use bootinfo::builder::Builder;
use bootinfo::item as bootitem;
use bootinfo::ItemKind;
use uefi::proto::fs::{OpenMode, SimpleFileSystem};
use uefi::proto::gop::{self, GraphicsOutput};
use uefi::proto::image::LoadedImage;
use uefi::table::{BootServices, BootTable};
use uefi::{u16cstr, Handle, Result, Status};

use crate::{elfload, page};

const BOOTINFO_FIXED_SIZE: usize = 0x1000;
const MMAP_EXTRA_ENTRIES: usize = 8;

pub struct BootinfoCtx {
    pub mmap_buf: &'static mut [MaybeUninit<u8>],
    pub builder: Builder<'static>,
}

pub fn load_kernel(image_handle: Handle, boot_services: &BootServices) -> Result<u64> {
    let loaded_image = boot_services.open_protocol::<LoadedImage>(image_handle, image_handle)?;

    let boot_fs = boot_services
        .open_protocol::<SimpleFileSystem>(loaded_image.device_handle(), image_handle)?;

    let root_dir = boot_fs.open_volume()?;
    let mut file = root_dir.open(u16cstr!("corrosios\\kernel"), OpenMode::READ)?;

    elfload::load_elf(boot_services, &mut file)
}

pub fn prepare_bootinfo(boot_table: &BootTable) -> Result<BootinfoCtx> {
    let boot_services = boot_table.boot_services();

    let framebuffer = get_framebuffer(boot_table)?;

    let (mmap_size, desc_size) = boot_services.memory_map_size()?;
    let max_mmap_entries = mmap_size / desc_size + MMAP_EXTRA_ENTRIES;

    let mut bootinfo_builder = make_bootinfo_builder(boot_services, max_mmap_entries)?;

    append_bootinfo(&mut bootinfo_builder, ItemKind::FRAMEBUFFER, framebuffer)?;

    Ok(BootinfoCtx {
        mmap_buf: alloc_uninit_bytes(boot_services, max_mmap_entries * desc_size)?,
        builder: bootinfo_builder,
    })
}

fn get_framebuffer(boot_table: &BootTable) -> Result<bootitem::Framebuffer> {
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

    Ok(bootitem::Framebuffer {
        paddr: gop_framebuffer.base as usize,
        size: gop_framebuffer.size,
        width: mode_info.hres,
        height: mode_info.vres,
        stride: mode_info.pixels_per_scanline,
        format,
    })
}

fn append_bootinfo<T>(builder: &mut Builder, kind: ItemKind, val: T) -> Result<()> {
    builder
        .append(kind, val)
        .map_err(|_| Status::OUT_OF_RESOURCES)
}

fn make_bootinfo_builder(
    boot_services: &BootServices,
    max_mmap_entries: usize,
) -> Result<Builder<'static>> {
    let buf = alloc_uninit_pages(
        boot_services,
        BOOTINFO_FIXED_SIZE + max_mmap_entries / mem::size_of::<bootitem::MemoryRange>(),
    )?;
    Ok(Builder::new(buf.as_out()).expect("buffer should be large and aligned"))
}

fn alloc_uninit_pages(
    boot_services: &BootServices,
    bytes: usize,
) -> Result<&'static mut [MaybeUninit<u8>]> {
    let p = page::alloc_pages(boot_services, bytes)?;
    Ok(unsafe { &mut *(p.as_ptr() as *mut _) })
}

fn alloc_uninit_bytes(
    boot_services: &BootServices,
    bytes: usize,
) -> Result<&'static mut [MaybeUninit<u8>]> {
    let p = boot_services.alloc(bytes)?;
    Ok(unsafe { slice::from_raw_parts_mut(p.cast(), bytes) })
}
