use core::ops::{Index, IndexMut};

use bitflags::bitflags;

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const LEVEL_SHIFT: usize = 9;
pub const ENTRY_COUNT: usize = 1 << LEVEL_SHIFT;

const PADDR_MASK: u64 = 0xffffffffff000;

bitflags! {
    pub struct EntryFlags: u64 {
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
pub struct Entry(u64);

impl Entry {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn from_paddr_flags(paddr: u64, flags: EntryFlags) -> Self {
        assert!(paddr & !PADDR_MASK == 0, "invalid physical frame address");
        Self(paddr | flags.bits())
    }

    pub const fn flags(self) -> EntryFlags {
        EntryFlags::from_bits_truncate(self.0)
    }

    pub const fn paddr(self) -> u64 {
        self.0 & PADDR_MASK
    }
}

#[repr(C, align(0x1000))]
pub struct PageTable {
    entries: [Entry; ENTRY_COUNT],
}

impl PageTable {
    pub const fn new() -> Self {
        Self {
            entries: [Entry::new(); ENTRY_COUNT],
        }
    }
}

impl Index<usize> for PageTable {
    type Output = Entry;

    fn index(&self, index: usize) -> &Entry {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Entry {
        &mut self.entries[index]
    }
}
