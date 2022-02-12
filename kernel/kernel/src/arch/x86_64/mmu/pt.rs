use core::ops::{Index, IndexMut};

use bitflags::bitflags;

use crate::mm::types::PhysPageNum;

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const PT_LEVEL_COUNT: usize = 4;

pub const PT_LEVEL_SHIFT: usize = 9;
pub const PT_ENTRY_COUNT: usize = 1 << PT_LEVEL_SHIFT;
pub const PT_LEVEL_MASK: usize = PT_ENTRY_COUNT - 1;

const PADDR_MASK: u64 = 0xffffffffff000;

bitflags! {
    pub struct PageTableFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_MODE = 1 << 2;

        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const HUGE_PAGE = 1 << 7;

        const NO_EXEC = 1 << 63;
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn new(page: PhysPageNum, flags: PageTableFlags) -> Self {
        Self(page.addr().as_u64() | flags.bits())
    }

    pub const fn flags(self) -> PageTableFlags {
        PageTableFlags::from_bits_truncate(self.0)
    }

    pub const fn is_present(self) -> bool {
        self.flags().contains(PageTableFlags::PRESENT)
    }

    pub const fn page(self) -> PhysPageNum {
        PhysPageNum::new((self.0 >> PAGE_SHIFT) as usize)
    }
}

#[derive(Clone, Copy)]
#[repr(C, align(0x1000))]
pub struct PageTable {
    entries: [PageTableEntry; PT_ENTRY_COUNT],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::empty(); PT_ENTRY_COUNT],
        }
    }
}

impl Index<usize> for PageTable {
    type Output = PageTableEntry;

    fn index(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
}
