use core::fmt;
use core::ops::{Index, IndexMut};

use bitflags::bitflags;

use crate::mm::types::{PageTableFlags, PageTablePerms, PhysPageNum};

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const PT_LEVEL_COUNT: usize = 4;

pub const PT_LEVEL_SHIFT: usize = 9;
pub const PT_ENTRY_COUNT: usize = 1 << PT_LEVEL_SHIFT;
pub const PT_LEVEL_MASK: usize = PT_ENTRY_COUNT - 1;

const PADDR_MASK: u64 = (1u64 << 52) - 1;

pub fn supports_page_size(level: usize) -> bool {
    matches!(level, 0 | 1)
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn new(page: PhysPageNum, perms: PageTablePerms, flags: PageTableFlags) -> Self {
        let mut x86_flags = X86PageTableFlags::empty();

        x86_flags.set(
            X86PageTableFlags::WRITABLE,
            perms.contains(PageTablePerms::WRITE),
        );
        x86_flags.set(
            X86PageTableFlags::USER_MODE,
            perms.contains(PageTablePerms::USER),
        );
        x86_flags.set(
            X86PageTableFlags::NO_EXEC,
            !perms.contains(PageTablePerms::EXECUTE),
        );

        x86_flags.set(
            X86PageTableFlags::PRESENT,
            flags.contains(PageTableFlags::PRESENT),
        );
        x86_flags.set(
            X86PageTableFlags::LARGE,
            flags.contains(PageTableFlags::LARGE),
        );

        Self(page.addr().as_u64() | x86_flags.bits())
    }

    pub fn perms(self) -> PageTablePerms {
        let flags = self.x86_flags();
        let mut ret = PageTablePerms::READ;

        ret.set(
            PageTablePerms::WRITE,
            flags.contains(X86PageTableFlags::WRITABLE),
        );
        ret.set(
            PageTablePerms::USER,
            flags.contains(X86PageTableFlags::USER_MODE),
        );
        ret.set(
            PageTablePerms::EXECUTE,
            !flags.contains(X86PageTableFlags::NO_EXEC),
        );

        ret
    }

    pub fn flags(self) -> PageTableFlags {
        let flags = self.x86_flags();
        let mut ret = PageTableFlags::empty();

        ret.set(
            PageTableFlags::PRESENT,
            flags.contains(X86PageTableFlags::PRESENT),
        );
        ret.set(
            PageTableFlags::LARGE,
            flags.contains(X86PageTableFlags::LARGE),
        );

        ret
    }

    pub const fn page(self) -> PhysPageNum {
        PhysPageNum::new(((self.0 & PADDR_MASK) >> PAGE_SHIFT) as usize)
    }

    const fn x86_flags(self) -> X86PageTableFlags {
        X86PageTableFlags::from_bits_truncate(self.0)
    }
}

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PageTableEntry")
            .field("page", &self.page().as_usize())
            .field("perms", &self.perms())
            .field("flags", &self.flags())
            .finish()
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

bitflags! {
    struct X86PageTableFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_MODE = 1 << 2;

        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const LARGE = 1 << 7;

        const NO_EXEC = 1 << 63;
    }
}
