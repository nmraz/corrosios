use crate::arch::mmu::{
    self, PageTable, PageTableEntry, PT_ENTRY_COUNT, PT_LEVEL_COUNT, PT_LEVEL_SHIFT,
};

use super::types::{PageTableFlags, PageTablePerms, PhysPageNum, VirtPageNum};

#[derive(Debug, Clone, Copy)]
pub struct PageTableAllocError;

#[derive(Debug, Clone, Copy)]
pub enum MapError {
    AllocFailed,
    EntryExists,
}

impl From<PageTableAllocError> for MapError {
    fn from(_: PageTableAllocError) -> Self {
        Self::AllocFailed
    }
}

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

pub struct MappingPointer {
    base: VirtPageNum,
    size: usize,
    offset: usize,
}

impl MappingPointer {
    pub fn new(base: VirtPageNum, size: usize) -> Self {
        Self {
            base,
            size,
            offset: 0,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn virt(&self) -> VirtPageNum {
        self.base + self.offset
    }

    pub fn remaining_pages(&self) -> usize {
        self.size - self.offset
    }

    pub fn advance(&mut self, pages: usize) {
        self.offset += pages;
        debug_assert!(self.offset <= self.size);
    }
}

pub struct Mapper<'a, A, T> {
    root_pt: &'a mut PageTable,
    inner: MapperInner<'a, A, T>,
}

impl<'a, A: PageTableAlloc, T: TranslatePhys> Mapper<'a, A, T> {
    /// # Safety
    ///
    /// The caller must guarantee that the provided table is correctly structured and that
    /// `translator` provides correct virtual page numbers for any queried physical pages.
    pub unsafe fn new(root_pt: &'a mut PageTable, alloc: &'a mut A, translator: T) -> Self {
        Self {
            root_pt,
            inner: MapperInner::new(alloc, translator),
        }
    }

    pub fn map_contiguous(
        &mut self,
        pointer: &mut MappingPointer,
        phys_base: PhysPageNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        self.inner
            .map_contiguous(self.root_pt, PT_LEVEL_COUNT - 1, pointer, phys_base, perms)?;
        Ok(())
    }
}

enum NextTableError {
    NotPresent,
    LargePage(PageTableEntry),
}

struct MapperInner<'a, A, T> {
    alloc: &'a mut A,
    translator: T,
}

impl<'a, A: PageTableAlloc, T: TranslatePhys> MapperInner<'a, A, T> {
    fn new(alloc: &'a mut A, translator: T) -> Self {
        Self { alloc, translator }
    }

    fn map_contiguous(
        &mut self,
        table: &mut PageTable,
        level: usize,
        pointer: &mut MappingPointer,
        phys_base: PhysPageNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        let mut index = pointer.virt().pt_index(level);

        while index < PT_ENTRY_COUNT && pointer.remaining_pages() > 0 {
            if mmu::supports_page_size(level) && can_use_level_page(level, pointer, phys_base) {
                let flags = if level == 0 {
                    PageTableFlags::empty()
                } else {
                    PageTableFlags::LARGE
                };

                self.map_terminal(table, index, phys_base + pointer.offset(), perms, flags)?;
                pointer.advance(level_page_count(level));
            } else {
                let next = self.next_table_or_create(table, index)?;
                self.map_contiguous(next, level - 1, pointer, phys_base, perms)?;
            }

            index += 1;
        }

        Ok(())
    }

    fn map_terminal(
        &mut self,
        table: &mut PageTable,
        index: usize,
        phys: PhysPageNum,
        perms: PageTablePerms,
        flags: PageTableFlags,
    ) -> Result<(), MapError> {
        let target_entry = &mut table[index];
        if target_entry.flags().contains(PageTableFlags::PRESENT) {
            return Err(MapError::EntryExists);
        }

        *target_entry = PageTableEntry::new(phys, perms, flags | PageTableFlags::PRESENT);

        Ok(())
    }

    fn next_table_or_create<'t>(
        &mut self,
        table: &'t mut PageTable,
        index: usize,
    ) -> Result<&'t mut PageTable, MapError> {
        let perms: PageTablePerms = PageTablePerms::READ
            | PageTablePerms::WRITE
            | PageTablePerms::EXECUTE
            | PageTablePerms::USER;

        match self.next_table_ptr(table, index) {
            Ok(next) => return Ok(unsafe { &mut *next }),

            Err(NextTableError::LargePage(_)) => return Err(MapError::EntryExists),
            Err(NextTableError::NotPresent) => {}
        };

        let new_table = self.alloc.allocate()?;
        table[index] = PageTableEntry::new(new_table, perms, PageTableFlags::PRESENT);
        Ok(unsafe { &mut *self.translate(new_table) })
    }

    fn next_table_ptr(
        &self,
        table: &PageTable,
        index: usize,
    ) -> Result<*mut PageTable, NextTableError> {
        let entry = table[index];
        let flags = entry.flags();

        if !flags.contains(PageTableFlags::PRESENT) {
            return Err(NextTableError::NotPresent);
        }

        if flags.contains(PageTableFlags::LARGE) {
            return Err(NextTableError::LargePage(entry));
        }

        Ok(self.translate(entry.page()))
    }

    fn translate(&self, table_pfn: PhysPageNum) -> *mut PageTable {
        self.translator.translate(table_pfn).addr().as_mut_ptr()
    }
}

fn can_use_level_page(level: usize, pointer: &MappingPointer, phys_base: PhysPageNum) -> bool {
    let min_pages = level_page_count(level);
    pointer.remaining_pages() >= min_pages
        && aligned_for_level(pointer.virt().as_usize(), level)
        && aligned_for_level(phys_base.as_usize() + pointer.offset(), level)
}

fn aligned_for_level(page_num: usize, level: usize) -> bool {
    page_num & level_page_mask(level) == 0
}

fn level_page_count(level: usize) -> usize {
    1 << (level * PT_LEVEL_SHIFT)
}

fn level_page_mask(level: usize) -> usize {
    level_page_count(level) - 1
}
