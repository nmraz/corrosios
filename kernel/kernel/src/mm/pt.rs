//! Low-level page table manipulation and traversal.
//!
//! This module should generally not be used directly; it is used by early initialization code and
//! by the VM subsystem to implement address spaces.

use core::{cmp, result};

use crate::arch::mmu::{
    self, get_pte_frame, make_empty_pte, make_pte, pte_is_present, pte_is_terminal, PageTableEntry,
    PT_ENTRY_COUNT, PT_LEVEL_COUNT, PT_LEVEL_SHIFT,
};
use crate::err::{Error, Result};

use super::types::{PageTablePerms, PhysFrameNum, VirtPageNum};

/// An object that can translate physical frame numbers to virtual page numbers that can be used to
/// access them.
pub trait TranslatePhys {
    /// Translates `phys` to a virtual page number that can be used to access it.
    fn translate(&self, phys: PhysFrameNum) -> VirtPageNum;
}

/// An allocator responsible for allocating physical frames for use as page tables.
pub trait PageTableAlloc {
    /// Allocates a new page table, returning its PFN.
    fn allocate(&mut self) -> Result<PhysFrameNum>;
}

/// Trait used to notify implementors that mappings have been updated and the TLB should be flushed.
pub trait GatherInvalidations {
    /// Notifies the implementor of the trait that the mapping for `vpn` has been modified and
    /// should be flushed from the TLB.
    fn add_tlb_flush(&mut self, vpn: VirtPageNum);
}

/// A virtual page range along with a progress pointer within it.
///
/// This is the structure used to track virtual page ranges in all map/unmap operations. It enables
/// those operations to report partial progress back to the caller even if they encounter an error
/// in the middle of the operation.
pub struct MappingPointer {
    base: VirtPageNum,
    size: usize,
    offset: usize,
}

impl MappingPointer {
    /// Creates a new mapping pointer spanning the page range `base..base + size`, with the pointer
    /// set to the start of the range.
    pub fn new(base: VirtPageNum, size: usize) -> Self {
        Self {
            base,
            size,
            offset: 0,
        }
    }

    /// Returns the current offset of this mapping pointer, measured in pages from the base.
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// Returns the current virtual page number pointed to by this mapping pointer.
    pub fn virt(&self) -> VirtPageNum {
        self.base + self.offset
    }

    /// Returns the number of pages remaining in the range, past the current offset.
    pub fn remaining_pages(&self) -> usize {
        self.size - self.offset
    }

    /// Advances the pointer forward by `pages`.
    ///
    /// The provided size should not cause the offset to exceed the total page count of the mapping
    /// pointer.
    pub fn advance(&mut self, pages: usize) {
        self.offset += pages;
        debug_assert!(self.offset <= self.size);
    }

    /// Advances the pointer forward by at most `pages`, or less if there are less than `pages`
    /// pages remaining.
    pub fn advance_clamped(&mut self, pages: usize) {
        self.offset = cmp::min(self.offset + pages, self.size);
    }
}

/// Structure for accessing and manipulating page tables.
pub struct PageTable<T> {
    root: PhysFrameNum,
    inner: PageTableInner<T>,
}

impl<T: TranslatePhys> PageTable<T> {
    /// Creates a new page table accessor for a page table rooted at `root_pt`, using `translator`
    /// to translate physical frames to virtual page numbers when necessary during traversal and
    /// manipulation.
    ///
    /// # Safety
    ///
    /// The caller must guarantee that the provided table is correctly structured and that
    /// `translator` provides correct virtual page numbers for any queried physical frames.
    pub unsafe fn new(root_pt: PhysFrameNum, translator: T) -> Self {
        Self {
            root: root_pt,
            inner: PageTableInner::new(translator),
        }
    }

    /// Maps the virtual page range spanned by `pointer` to a contiguous physical range starting at
    /// `phys_base`, with permissions `perms`.
    ///
    /// This function does not support overwriting existing mappings, and will fail if it encounters
    /// a page that is already mapped.
    ///
    /// When this function returns, `pointer` will point past the last page mapped successfully. On
    /// success, this will always be the last page, but if the function returns early due to an
    /// error, the reported progress can be used to take appropriate action.
    ///
    /// # Errors
    ///
    /// * `OUT_OF_MEMORY` - A page table allocation failed.
    /// * `RESOURCE_OVERLAP` - A page in the range was already mapped.
    ///
    /// # Safety
    ///
    /// * The page table must not be accessed concurrently by other cores/interrupts during the
    ///   mapping
    /// * The provided allocator must return physical frames usable as page tables
    pub unsafe fn map(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        pointer: &mut MappingPointer,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<()> {
        self.inner.map(
            alloc,
            pointer,
            self.root,
            PT_LEVEL_COUNT - 1,
            phys_base,
            perms,
        )
    }

    /// Unmaps any pages in the range covered by `pointer`, reporting any virtual pages that need
    /// TLB invalidation to `gather`.
    ///
    /// This function will skip any unmapped "holes" encountered in the range.
    ///
    /// This function currently cannot split large pages, and will return an error if the range
    /// partially intersects one.
    ///
    /// When this function returns, `pointer` will point past the last page unmapped successfully.
    /// On success, this will always be the last page, but if the function returns early due to an
    /// error, the reported progress can be used to take appropriate action.
    ///
    /// # Errors
    ///
    /// * `RESOURCE_OVERLAP` - The unmapping range partially intersected a large page.
    ///
    /// # Safety
    ///
    /// * The page table must not be accessed concurrently by other cores/interrupts during the
    ///   unmapping
    /// * Any cores on which the page table is active must not access the virtual addresses unmapped
    ///   by the call
    pub unsafe fn unmap(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
    ) -> Result<()> {
        self.inner
            .unmap(gather, pointer, self.root, PT_LEVEL_COUNT - 1)
    }
}

enum NextTableError {
    NotPresent,
    LargePage(PageTableEntry),
}

struct PageTableInner<T> {
    translator: T,
}

impl<T: TranslatePhys> PageTableInner<T> {
    fn new(translator: T) -> Self {
        Self { translator }
    }

    fn map(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<()> {
        walk_level(level, pointer, |pointer| {
            if mmu::supports_page_size(level) && can_use_level_page(level, pointer, phys_base) {
                self.map_terminal(pointer, table, level, phys_base, perms)?;
            } else {
                let next =
                    self.next_table_or_create(alloc, table, pointer.virt().pt_index(level), level)?;
                self.map(alloc, pointer, next, level - 1, phys_base, perms)?;
            }

            Ok(())
        })
    }

    fn unmap(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
    ) -> Result<()> {
        walk_level(level, pointer, |pointer| {
            if level == 0 {
                self.unmap_terminal(gather, pointer, table, level);
            } else {
                let index = pointer.virt().pt_index(level);
                let next = match self.next_table(table, index, level) {
                    Ok(next_ptr) => next_ptr,

                    Err(NextTableError::LargePage(_entry)) => {
                        let page_count = level_page_count(level);

                        if aligned_for_level(pointer.virt().as_usize(), level)
                            && pointer.remaining_pages() >= page_count
                        {
                            self.unmap_terminal(gather, pointer, table, level);
                            return Ok(());
                        } else {
                            return Err(Error::RESOURCE_OVERLAP);
                        }
                    }

                    Err(NextTableError::NotPresent) => {
                        pointer.advance_clamped(level_page_count(level));
                        return Ok(());
                    }
                };

                self.unmap(gather, pointer, next, level - 1)?;
            }

            Ok(())
        })
    }

    fn next_table_or_create(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        table: PhysFrameNum,
        index: usize,
        level: usize,
    ) -> Result<PhysFrameNum> {
        let perms: PageTablePerms = PageTablePerms::READ
            | PageTablePerms::WRITE
            | PageTablePerms::EXECUTE
            | PageTablePerms::USER;

        match self.next_table(table, index, level) {
            Ok(next) => return Ok(next),
            Err(NextTableError::LargePage(_)) => return Err(Error::RESOURCE_OVERLAP),
            Err(NextTableError::NotPresent) => {}
        };

        let new_table = alloc.allocate()?;
        self.clear_table(new_table);
        self.set(table, index, make_pte(level, false, new_table, perms));

        Ok(new_table)
    }

    fn next_table(
        &self,
        table: PhysFrameNum,
        index: usize,
        level: usize,
    ) -> result::Result<PhysFrameNum, NextTableError> {
        let pte = self.get(table, index);

        if !pte_is_present(pte, level) {
            return Err(NextTableError::NotPresent);
        }

        if pte_is_terminal(pte, level) {
            return Err(NextTableError::LargePage(pte));
        }

        Ok(get_pte_frame(pte, level))
    }

    fn map_terminal(
        &mut self,
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
    ) -> Result<()> {
        let index = pointer.virt().pt_index(level);

        if pte_is_present(self.get(table, index), level) {
            return Err(Error::RESOURCE_OVERLAP);
        }

        self.set(
            table,
            index,
            make_pte(level, true, phys_base + pointer.offset(), perms),
        );

        pointer.advance(level_page_count(level));

        Ok(())
    }

    fn unmap_terminal(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
    ) {
        self.set(table, pointer.virt().pt_index(level), make_empty_pte());
        gather.add_tlb_flush(pointer.virt());
        pointer.advance(level_page_count(level));
    }

    fn get(&self, table: PhysFrameNum, index: usize) -> PageTableEntry {
        let entry_ptr = self.entry(table, index);
        unsafe { entry_ptr.read() }
    }

    fn clear_table(&mut self, table: PhysFrameNum) {
        let table_virt: *mut PageTableEntry = self.translator.translate(table).addr().as_mut_ptr();
        unsafe {
            for i in 0..PT_ENTRY_COUNT {
                table_virt.add(i).write(make_empty_pte());
            }
        }
    }

    fn set(&mut self, table: PhysFrameNum, index: usize, entry: PageTableEntry) {
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
    mut f: impl FnMut(&mut MappingPointer) -> result::Result<(), E>,
) -> result::Result<(), E> {
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
