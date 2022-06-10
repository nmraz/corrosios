use core::cmp;

use crate::arch::mmu::{self, PageTableEntry, PT_ENTRY_COUNT, PT_LEVEL_COUNT, PT_LEVEL_SHIFT};

use super::types::{PageTableFlags, PageTablePerms, PhysFrameNum, VirtPageNum};

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
    fn allocate(&mut self) -> Result<PhysFrameNum, PageTableAllocError>;
}

pub trait TranslatePhys {
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum;
}

pub trait GatherInvalidations {
    fn add_tlb_flush(&mut self, vpn: VirtPageNum);
    fn add_pt_dealloc(&mut self, pt: PhysFrameNum);
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
    root_pt: PhysFrameNum,
    inner: MapperInner<'a, A, T>,
}

impl<'a, A: PageTableAlloc, T: TranslatePhys> Mapper<'a, A, T> {
    /// # Safety
    ///
    /// The caller must guarantee that the provided table is correctly structured and that
    /// `translator` provides correct virtual page numbers for any queried physical pages.
    pub unsafe fn new(root_pt: PhysFrameNum, alloc: &'a mut A, translator: T) -> Self {
        Self {
            root_pt,
            inner: MapperInner::new(alloc, translator),
        }
    }

    pub fn map(
        &mut self,
        pointer: &mut MappingPointer,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        self.inner
            .map(self.root_pt, PT_LEVEL_COUNT - 1, pointer, phys_base, perms)
    }

    pub fn unmap(
        &mut self,
        pointer: &mut MappingPointer,
        gather: &mut impl GatherInvalidations,
    ) -> Result<(), PageTableAllocError> {
        self.inner
            .unmap(self.root_pt, PT_LEVEL_COUNT - 1, pointer, gather)
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

    fn map(
        &mut self,
        table: PhysFrameNum,
        level: usize,
        pointer: &mut MappingPointer,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        walk_level(level, pointer, |pointer| {
            if mmu::supports_page_size(level) && can_use_level_page(level, pointer, phys_base) {
                self.map_terminal(table, level, pointer, phys_base, perms)?;
            } else {
                let next = self.next_table_or_create(table, pointer.virt().pt_index(level))?;
                self.map(next, level - 1, pointer, phys_base, perms)?;
            }

            Ok(())
        })
    }

    fn unmap(
        &mut self,
        table: PhysFrameNum,
        level: usize,
        pointer: &mut MappingPointer,
        gather: &mut dyn GatherInvalidations,
    ) -> Result<(), PageTableAllocError> {
        walk_level(level, pointer, |pointer| {
            if level == 0 {
                self.unmap_terminal(table, level, pointer, gather);
            } else {
                let index = pointer.virt().pt_index(level);
                let next = match self.next_table(table, index) {
                    Ok(next_ptr) => next_ptr,

                    Err(NextTableError::LargePage(entry)) => {
                        let page_count = level_page_count(level);

                        if aligned_for_level(pointer.virt().as_usize(), level)
                            && pointer.remaining_pages() >= page_count
                        {
                            self.unmap_terminal(table, level, pointer, gather);
                            return Ok(());
                        } else {
                            todo!("Split large page")
                        }
                    }

                    Err(NextTableError::NotPresent) => {
                        pointer.advance(level_page_count(level));
                        return Ok(());
                    }
                };

                self.unmap(next, level - 1, pointer, gather)?;
            }

            Ok(())
        })

        // TODO: free table if possible
    }

    fn next_table_or_create(
        &mut self,
        table: PhysFrameNum,
        index: usize,
    ) -> Result<PhysFrameNum, MapError> {
        let perms: PageTablePerms = PageTablePerms::READ
            | PageTablePerms::WRITE
            | PageTablePerms::EXECUTE
            | PageTablePerms::USER;

        match self.next_table(table, index) {
            Ok(next) => return Ok(next),
            Err(NextTableError::LargePage(_)) => return Err(MapError::EntryExists),
            Err(NextTableError::NotPresent) => {}
        };

        let new_table = self.alloc.allocate()?;
        self.set(
            table,
            index,
            PageTableEntry::new(new_table, perms, PageTableFlags::PRESENT),
        );

        Ok(new_table)
    }

    fn next_table(
        &self,
        table: PhysFrameNum,
        index: usize,
    ) -> Result<PhysFrameNum, NextTableError> {
        let entry = self.get(table, index);
        let flags = entry.flags();

        if !flags.contains(PageTableFlags::PRESENT) {
            return Err(NextTableError::NotPresent);
        }

        if flags.contains(PageTableFlags::LARGE) {
            return Err(NextTableError::LargePage(entry));
        }

        Ok(entry.page())
    }

    fn map_terminal(
        &self,
        table: PhysFrameNum,
        level: usize,
        pointer: &mut MappingPointer,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        let index = pointer.virt().pt_index(level);

        if self
            .get(table, index)
            .flags()
            .contains(PageTableFlags::PRESENT)
        {
            return Err(MapError::EntryExists);
        }

        let mut flags = PageTableFlags::PRESENT;
        if level > 0 {
            flags |= PageTableFlags::LARGE;
        }

        self.set(
            table,
            index,
            PageTableEntry::new(phys_base + pointer.offset(), perms, flags),
        );

        pointer.advance(level_page_count(level));

        Ok(())
    }

    fn unmap_terminal(
        &self,
        table: PhysFrameNum,
        level: usize,
        pointer: &mut MappingPointer,
        gather: &mut dyn GatherInvalidations,
    ) {
        self.set(
            table,
            pointer.virt().pt_index(level),
            PageTableEntry::empty(),
        );
        gather.add_tlb_flush(pointer.virt());
        pointer.advance(level_page_count(level));
    }

    fn get(&self, table: PhysFrameNum, index: usize) -> PageTableEntry {
        let entry_ptr = self.entry(table, index);
        unsafe { entry_ptr.read() }
    }

    fn set(&self, table: PhysFrameNum, index: usize, entry: PageTableEntry) {
        let entry_ptr = self.entry(table, index);
        unsafe {
            entry_ptr.write_volatile(entry);
        }
    }

    fn entry(&self, table: PhysFrameNum, index: usize) -> *mut PageTableEntry {
        assert!(index < PT_ENTRY_COUNT, "page table access out of bounds");
        unsafe {
            self.translator
                .translate(table)
                .addr()
                .as_mut_ptr::<PageTableEntry>()
                .add(index)
        }
    }
}

fn walk_level<E>(
    level: usize,
    pointer: &mut MappingPointer,
    mut f: impl FnMut(&mut MappingPointer) -> Result<(), E>,
) -> Result<(), E> {
    let virt = pointer.virt();
    let range_end = virt + pointer.remaining_pages();
    let next_table_boundary = (virt + 1).align_up(PT_ENTRY_COUNT * level_page_count(level));

    let max_virt = cmp::min(range_end, next_table_boundary);

    while pointer.virt() < max_virt {
        f(pointer)?;
    }

    Ok(())
}

fn can_use_level_page(level: usize, pointer: &MappingPointer, phys_base: PhysFrameNum) -> bool {
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
