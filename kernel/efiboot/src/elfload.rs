use alloc::vec::Vec;
use core::mem::{self, MaybeUninit};
use core::{cmp, iter, slice};

use minielf::{Header, ProgramHeader, SEGMENT_TYPE_LOAD};
use uefi::proto::fs::File;
use uefi::table::BootServices;
use uefi::{BootAlloc, Result, Status};

use uninit::extension_traits::AsOut;

use crate::page::{self, PAGE_SIZE};

pub fn load_elf(boot_services: &BootServices, file: &mut File<'_>) -> Result<u64> {
    let header = read_header(file)?;
    let pheaders = read_pheaders(boot_services, &header, file)?;

    let loadable = pheaders
        .iter()
        .filter(|pheader| pheader.ty == SEGMENT_TYPE_LOAD);

    let entry_covered = loadable.clone().any(|pheader| {
        (pheader.phys_addr..pheader.phys_addr + pheader.mem_size).contains(&header.entry)
    });

    if !entry_covered {
        return Err(Status::LOAD_ERROR);
    }

    let (min_paddr, max_paddr, align) = loadable
        .clone()
        .map(|pheader| {
            (
                pheader.phys_addr,
                pheader.phys_addr + pheader.mem_size,
                pheader.align,
            )
        })
        .reduce(|(start1, end1, align1), (start2, end2, align2)| {
            (
                cmp::min(start1, start2),
                cmp::max(end1, end2),
                cmp::max(align1, align2),
            )
        })
        .ok_or(Status::LOAD_ERROR)?;

    if align > PAGE_SIZE as u64 {
        return Err(Status::LOAD_ERROR);
    }

    let buf = page::alloc_uninit_pages(boot_services, (max_paddr - min_paddr) as usize)?;

    for pheader in loadable {
        load_segment(buf, min_paddr, file, pheader)?;
    }

    Ok(header.entry - min_paddr + buf.as_ptr() as u64)
}

fn load_segment(
    buf: &mut [MaybeUninit<u8>],
    base_paddr: u64,
    file: &mut File<'_>,
    pheader: &ProgramHeader,
) -> Result<()> {
    if pheader.phys_addr as usize % PAGE_SIZE != 0 || pheader.file_size > pheader.mem_size {
        return Err(Status::LOAD_ERROR);
    }

    let phys_off = (pheader.phys_addr - base_paddr) as usize;
    let buf = &mut buf[phys_off..phys_off + pheader.mem_size as usize];

    let file_size = pheader.file_size as usize;
    let (file_part, bss_part) = buf.as_out().split_at_out(file_size);

    file.set_position(pheader.off)?;
    file.read_exact(file_part)?;

    bss_part.init_with(iter::repeat(0));

    Ok(())
}

fn read_pheaders<'b>(
    boot_services: &'b BootServices,
    header: &Header,
    file: &mut File<'_>,
) -> Result<Vec<ProgramHeader, BootAlloc<'b>>> {
    if header.ph_entry_size as usize != mem::size_of::<ProgramHeader>() {
        return Err(Status::LOAD_ERROR);
    }

    file.set_position(header.ph_off)?;

    let count = header.ph_entry_num as usize;
    let mut headers = Vec::with_capacity_in(count, BootAlloc::new(boot_services));

    unsafe {
        let buf = slice::from_raw_parts_mut(
            headers.as_mut_ptr() as *mut MaybeUninit<u8>,
            count * mem::size_of::<ProgramHeader>(),
        );
        file.read_exact(buf.as_out())?;
        headers.set_len(count);
    }

    Ok(headers)
}

fn read_header(file: &mut File<'_>) -> Result<Header> {
    file.set_position(0)?;
    let header: Header = unsafe { read(file)? };

    if header.is_valid() {
        Ok(header)
    } else {
        Err(Status::LOAD_ERROR)
    }
}

unsafe fn read<T>(file: &mut File<'_>) -> Result<T> {
    let mut val = MaybeUninit::uninit();
    let buf = unsafe {
        slice::from_raw_parts_mut(
            val.as_mut_ptr() as *mut MaybeUninit<u8>,
            mem::size_of::<T>(),
        )
    };

    file.read_exact(buf.as_out())?;

    Ok(unsafe { val.assume_init() })
}
