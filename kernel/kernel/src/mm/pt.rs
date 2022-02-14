use crate::arch::mmu::PageTableFlags;

use super::types::{PageTablePerms, PhysPageNum, VirtPageNum};

pub use map::{MapError, Mapper};

mod map;
mod walk;

#[derive(Debug, Clone, Copy)]
pub struct PageTableAllocError;

/// # Safety
///
/// The implementation must ensure that it returns memory usable as a page table along with its true
/// physical address.
pub unsafe trait PageTableAlloc {
    fn allocate(&mut self) -> Result<PhysPageNum, PageTableAllocError>;
    unsafe fn deallocate(&mut self, pfn: PhysPageNum);
}

pub trait TranslatePhys {
    fn translate(&self, phys: PhysPageNum) -> VirtPageNum;
}

fn flags_from_perms(perms: PageTablePerms) -> PageTableFlags {
    let mut flags = PageTableFlags::common();
    flags.apply_perms(perms);
    flags
}
