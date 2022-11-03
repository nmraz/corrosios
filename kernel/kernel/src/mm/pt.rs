//! Low-level page table manipulation and traversal.
//!
//! This module should generally not be used directly; it is used by early initialization code and
//! by the VM subsystem to implement address spaces.

use core::{cmp, result};

use crate::arch::mmu::{
    self, get_pte_frame, make_empty_pte, make_intermediate_pte, make_terminal_pte, pte_is_present,
    pte_is_terminal, update_pte_perms, PageTableEntry, PT_ENTRY_COUNT, PT_LEVEL_COUNT,
    PT_LEVEL_SHIFT,
};
use crate::err::{Error, Result};

use super::types::{CacheMode, PageTablePerms, PhysFrameNum, VirtPageNum};

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

/// [`GatherInvalidations`] implementation that does nothing.
///
/// This is useful when fine-grained invalidation tracking is not necessary, as the entire TLB will
/// be flushed anyway.
pub struct NoopGather;

impl GatherInvalidations for NoopGather {
    fn add_tlb_flush(&mut self, _vpn: VirtPageNum) {}
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
    /// `phys_base`, with permissions `perms` and cache mode `cache_mode`.
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
    /// * `cache_mode` must be a cache mode that can safely be applied to the provided pages,
    ///   respecting any platform limitations
    pub unsafe fn map(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        pointer: &mut MappingPointer,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
        cache_mode: CacheMode,
    ) -> Result<()> {
        self.inner.map(
            alloc,
            pointer,
            self.root,
            PT_LEVEL_COUNT - 1,
            phys_base,
            perms,
            cache_mode,
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
    /// * Any pages reported to `gather` must be flushed from the TLB before later attempts to
    ///   re-map them.
    pub unsafe fn unmap(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
    ) -> Result<()> {
        self.inner.walk_update(
            gather,
            pointer,
            &mut |_pte, _level| make_empty_pte(),
            self.root,
            PT_LEVEL_COUNT - 1,
        )
    }

    /// Updates the protection permissions of all pages in the range covered by `pointer`, reporting
    /// any virtual pages that need TLB invalidation to `gather`.
    ///
    /// This function will skip any "holes" encountered in the range.
    ///
    /// This function currently cannot split large pages, and will return an error if the range
    /// partially intersects one.
    ///
    /// When this function returns, `pointer` will point past the last page updated successfully.
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
    ///   operation
    /// * The caller must guarantee that any page faults caused by accesses after the protection has
    ///   been updated will be handled correctly.
    /// * Any pages reported to `gather` must be flushed from the TLB before the new permissions
    ///   can be relied on.
    pub unsafe fn protect(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
        perms: PageTablePerms,
    ) -> Result<()> {
        self.inner.walk_update(
            gather,
            pointer,
            &mut |pte, level| update_pte_perms(pte, level, perms),
            self.root,
            PT_LEVEL_COUNT - 1,
        )
    }

    /// Invokes `cull` on any nested page tables in the range `base..base + size` and unlinks them
    /// from their parents.
    ///
    /// # Safety
    ///
    /// * The page table must not be accessed concurrently by other cores/interrupts during the
    ///   operation
    pub unsafe fn cull_tables(
        &mut self,
        mut cull: impl FnMut(PhysFrameNum),
        base: VirtPageNum,
        size: usize,
    ) {
        self.inner.cull_tables(
            &mut cull,
            &mut MappingPointer::new(base, size),
            self.root,
            PT_LEVEL_COUNT - 1,
        );
    }
}

enum NextTableError {
    NotPresent,
    TerminalEntry(PageTableEntry),
}

struct PageTableInner<T> {
    translator: T,
}

impl<T: TranslatePhys> PageTableInner<T> {
    fn new(translator: T) -> Self {
        Self { translator }
    }

    #[allow(clippy::too_many_arguments)]
    fn map(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
        phys_base: PhysFrameNum,
        perms: PageTablePerms,
        cache_mode: CacheMode,
    ) -> Result<()> {
        walk_level(level, pointer, |pointer| {
            if mmu::supports_page_size(level) && can_use_level_page(level, pointer, phys_base) {
                self.map_terminal(pointer, table, level, phys_base, perms, cache_mode)?;
            } else {
                let next =
                    self.next_table_or_create(alloc, table, pointer.virt().pt_index(level), level)?;
                self.map(
                    alloc,
                    pointer,
                    next,
                    level - 1,
                    phys_base,
                    perms,
                    cache_mode,
                )?;
            }

            Ok(())
        })
    }

    fn walk_update(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
        update: &mut impl FnMut(PageTableEntry, usize) -> PageTableEntry,
        table: PhysFrameNum,
        level: usize,
    ) -> Result<()> {
        walk_level(level, pointer, |pointer| {
            if level == 0 {
                self.update_terminal(gather, pointer, update, table, level);
            } else {
                let index = pointer.virt().pt_index(level);
                let next = match self.next_table(table, index, level) {
                    Ok(next_ptr) => next_ptr,

                    Err(NextTableError::TerminalEntry(_entry)) => {
                        if covers_level_entry(pointer, level) {
                            self.update_terminal(gather, pointer, update, table, level);
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

                self.walk_update(gather, pointer, update, next, level - 1)?;
            }

            Ok(())
        })
    }

    fn cull_tables(
        &mut self,
        cull: &mut impl FnMut(PhysFrameNum),
        pointer: &mut MappingPointer,
        table: PhysFrameNum,
        level: usize,
    ) {
        walk_level(level, pointer, |pointer| -> result::Result<(), ()> {
            let next_table_covered = covers_level_entry(pointer, level);

            let index = pointer.virt().pt_index(level);
            let Ok(next) = self.next_table(table, index, level) else {
                pointer.advance_clamped(level_page_count(level));
                return Ok(());
            };

            if level > 1 {
                self.cull_tables(cull, pointer, next, level - 1);
            }

            if next_table_covered || self.table_is_empty(next, level - 1) {
                self.set(table, index, make_empty_pte());
                cull(next);
            }

            Ok(())
        })
        .unwrap();
    }

    fn next_table_or_create(
        &mut self,
        alloc: &mut impl PageTableAlloc,
        table: PhysFrameNum,
        index: usize,
        level: usize,
    ) -> Result<PhysFrameNum> {
        match self.next_table(table, index, level) {
            Ok(next) => return Ok(next),
            Err(NextTableError::TerminalEntry(_)) => return Err(Error::RESOURCE_OVERLAP),
            Err(NextTableError::NotPresent) => {}
        };

        let new_table = alloc.allocate()?;
        self.clear_table(new_table);
        self.set(table, index, make_intermediate_pte(level, new_table));

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
            return Err(NextTableError::TerminalEntry(pte));
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
        cache_mode: CacheMode,
    ) -> Result<()> {
        let index = pointer.virt().pt_index(level);

        if pte_is_present(self.get(table, index), level) {
            return Err(Error::RESOURCE_OVERLAP);
        }

        self.set(
            table,
            index,
            make_terminal_pte(level, phys_base + pointer.offset(), perms, cache_mode),
        );

        pointer.advance(level_page_count(level));

        Ok(())
    }

    fn update_terminal(
        &mut self,
        gather: &mut impl GatherInvalidations,
        pointer: &mut MappingPointer,
        update: &mut impl FnMut(PageTableEntry, usize) -> PageTableEntry,
        table: PhysFrameNum,
        level: usize,
    ) {
        let index = pointer.virt().pt_index(level);
        self.set(table, index, update(self.get(table, index), level));
        gather.add_tlb_flush(pointer.virt());
        pointer.advance(level_page_count(level));
    }

    fn table_is_empty(&self, table: PhysFrameNum, level: usize) -> bool {
        (0..PT_ENTRY_COUNT).all(|i| !pte_is_present(self.get(table, i), level))
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
    covers_level_entry(pointer, level)
        && aligned_for_level(phys_base.as_usize() + pointer.offset(), level)
}

fn covers_level_entry(pointer: &MappingPointer, level: usize) -> bool {
    aligned_for_level(pointer.virt().as_usize(), level)
        && pointer.remaining_pages() >= level_page_count(level)
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
