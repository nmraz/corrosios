use alloc::vec::Vec;
use core::mem::{self, MaybeUninit};
use core::slice;

use minielf::{Header, ProgramHeader, SEGMENT_TYPE_LOAD};
use uefi::proto::fs::File;
use uefi::table::{AllocMode, BootServices};
use uefi::{Result, Status};

const PAGE_SIZE: u64 = 0x1000;

pub fn load_elf(boot_services: &BootServices, file: &mut File<'_>) -> Result<u64> {
    let header = read_header(file)?;
    let pheaders = read_pheaders(&header, file)?;

    let loadable = pheaders
        .iter()
        .filter(|pheader| pheader.ty == SEGMENT_TYPE_LOAD);

    let entry_covered = loadable.clone().any(|pheader| {
        (pheader.phys_addr..pheader.phys_addr + pheader.mem_size).contains(&header.entry)
    });

    if !entry_covered {
        return Err(Status::LOAD_ERROR);
    }

    for pheader in loadable {
        load_segment(boot_services, pheader, file)?;
    }

    Ok(header.entry)
}

fn load_segment(
    boot_services: &BootServices,
    pheader: &ProgramHeader,
    file: &mut File<'_>,
) -> Result<()> {
    if pheader.phys_addr % PAGE_SIZE != 0 || pheader.file_size > pheader.mem_size {
        return Err(Status::LOAD_ERROR);
    }

    let pages = (pheader.mem_size + PAGE_SIZE - 1) / PAGE_SIZE;
    boot_services.alloc_pages(AllocMode::At(pheader.phys_addr), pages as usize)?;

    // Safety: memory range has been reserved via call to `alloc_pages` above.
    let buf = unsafe {
        slice::from_raw_parts_mut(pheader.phys_addr as *mut u8, pheader.mem_size as usize)
    };

    let file_size = pheader.file_size as usize;

    file.set_position(pheader.off)?;
    file.read_exact(&mut buf[..file_size])?;
    buf[file_size..].fill(0);

    Ok(())
}

fn read_pheaders(header: &Header, file: &mut File<'_>) -> Result<Vec<ProgramHeader>> {
    if header.ph_entry_size as usize != mem::size_of::<ProgramHeader>() {
        return Err(Status::LOAD_ERROR);
    }

    file.set_position(header.ph_off)?;

    let count = header.ph_entry_num as usize;
    let mut headers = Vec::with_capacity(count);

    unsafe {
        let buf = slice::from_raw_parts_mut(
            headers.as_mut_ptr() as *mut _,
            count * mem::size_of::<ProgramHeader>(),
        );
        file.read_exact(buf)?;
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
    let buf = unsafe { slice::from_raw_parts_mut(val.as_mut_ptr() as *mut _, mem::size_of::<T>()) };

    file.read_exact(buf)?;

    Ok(unsafe { val.assume_init() })
}