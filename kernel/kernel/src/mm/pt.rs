use crate::arch::mmu::{PageTable, PageTableEntry, PageTableFlags, PT_ENTRY_COUNT, PT_LEVEL_COUNT};

use super::types::{PageTablePerms, PhysPageNum, VirtPageNum};

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
        phys: PhysPageNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        // TODO: remove partial mappings on error
        self.inner
            .map_contiguous(self.root_pt, PT_LEVEL_COUNT - 1, pointer, phys, perms)?;

        // TODO: handle TLB

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
        mut phys: PhysPageNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        let mut index = pointer.virt().pt_index(level);

        while index < PT_ENTRY_COUNT && pointer.remaining_pages() > 0 {
            if level == 0 {
                self.map_terminal(table, index, phys, perms)?;
                phys += 1;
                pointer.advance(1);
            } else {
                // TODO: large page support?
                let next = self.next_table_or_create(table, index)?;
                self.map_contiguous(next, level - 1, pointer, phys, perms)?;
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
    ) -> Result<(), MapError> {
        let target_entry = &mut table[index];
        if target_entry.flags().has_present() {
            return Err(MapError::EntryExists);
        }

        *target_entry = PageTableEntry::new(phys, flags_from_perms(perms));

        Ok(())
    }

    fn next_table_or_create<'t>(
        &mut self,
        table: &'t mut PageTable,
        index: usize,
    ) -> Result<&'t mut PageTable, PageTableAllocError> {
        let perms: PageTablePerms = PageTablePerms::READ
            | PageTablePerms::WRITE
            | PageTablePerms::EXECUTE
            | PageTablePerms::USER;

        match self.next_table_ptr(table, index) {
            Ok(next) => return Ok(unsafe { &mut *next }),

            Err(NextTableError::LargePage(_)) => {
                panic!("encountered large page")
            }
            Err(NextTableError::NotPresent) => {}
        };

        let new_table = self.alloc.allocate()?;
        table[index] = PageTableEntry::new(new_table, flags_from_perms(perms));
        Ok(unsafe { &mut *self.translate(new_table) })
    }

    fn next_table_ptr(
        &self,
        table: &PageTable,
        index: usize,
    ) -> Result<*mut PageTable, NextTableError> {
        let entry = table[index];
        let flags = entry.flags();

        if !flags.has_present() {
            return Err(NextTableError::NotPresent);
        }

        if flags.has_large() {
            return Err(NextTableError::LargePage(entry));
        }

        Ok(self.translate(entry.page()))
    }

    fn translate(&self, table_pfn: PhysPageNum) -> *mut PageTable {
        self.translator.translate(table_pfn).addr().as_mut_ptr()
    }
}

fn flags_from_perms(perms: PageTablePerms) -> PageTableFlags {
    let mut flags = PageTableFlags::common();
    flags.apply_perms(perms);
    flags
}
