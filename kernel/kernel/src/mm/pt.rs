use core::ptr;

use crate::arch::mmu::{PageTable, PageTableEntry, PageTableFlags, PT_LEVEL_COUNT};

use super::types::{PageTablePerms, PhysPageNum, VirtPageNum};

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

pub struct Walker<T> {
    translator: T,
}

impl<T: TranslatePhys> Walker<T> {
    /// # Safety
    ///
    /// The caller must guarantee that `translator` provides correct virtual page numbers for any
    /// queried physical pages.
    pub unsafe fn new(translator: T) -> Self {
        Self { translator }
    }

    /// # Safety
    ///
    /// The provided table must be correctly structured.
    pub unsafe fn next_table<'a>(
        &self,
        table: &'a PageTable,
        index: usize,
    ) -> Option<&'a PageTable> {
        self.next_table_ptr(table, index).map(|p| unsafe { &*p })
    }

    /// # Safety
    ///
    /// The provided table must be correctly structured.
    pub unsafe fn next_table_mut<'a>(
        &self,
        table: &'a mut PageTable,
        index: usize,
    ) -> Option<&'a mut PageTable> {
        self.next_table_ptr(table, index)
            .map(|p| unsafe { &mut *p })
    }

    /// # Safety
    ///
    /// The provided table must be correctly structured.
    pub unsafe fn next_table_mut_ensure_perms<'a>(
        &self,
        table: &'a mut PageTable,
        index: usize,
        perms: PageTablePerms,
    ) -> Option<&'a mut PageTable> {
        self.next_table_ptr_ensure_perms(table, index, perms)
            .map(|p| unsafe { &mut *p })
    }

    /// # Safety
    ///
    /// The provided table must be correctly structured.
    pub unsafe fn next_table_or_create<'a, 'b, A: PageTableAlloc>(
        &self,
        table: &'a mut PageTable,
        index: usize,
        alloc: &'b mut A,
        perms: PageTablePerms,
    ) -> Result<&'a mut PageTable, PageTableAllocError> {
        if let Some(next) = self.next_table_ptr_ensure_perms(table, index, perms) {
            return Ok(unsafe { &mut *next });
        }

        let new_table_pfn = alloc.allocate()?;
        let new_table = self.translator.translate(new_table_pfn).addr().as_mut_ptr();
        unsafe {
            ptr::write(new_table, PageTable::new());
        }

        table[index] = PageTableEntry::new(new_table_pfn, flags_from_perms(perms));
        Ok(unsafe { &mut *new_table })
    }

    fn next_table_ptr(&self, table: &PageTable, index: usize) -> Option<*mut PageTable> {
        let entry = table[index];
        entry.flags().has_present().then(|| {
            assert!(
                !entry.flags().has_huge(),
                "attempting to walk through huge page"
            );
            self.translator.translate(entry.page()).addr().as_mut_ptr()
        })
    }

    fn next_table_ptr_ensure_perms(
        &self,
        table: &mut PageTable,
        index: usize,
        perms: PageTablePerms,
    ) -> Option<*mut PageTable> {
        let entry = &mut table[index];
        let mut flags = entry.flags();
        if !flags.has_present() {
            return None;
        }

        assert!(!flags.has_huge(), "attempting to walk through huge page");

        flags.add_perms(perms);

        let pfn = entry.page();
        *entry = PageTableEntry::new(pfn, flags);

        Some(self.translator.translate(pfn).addr().as_mut_ptr())
    }
}

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

pub struct Mapper<'a, A, T> {
    root_pt: &'a mut PageTable,
    alloc: &'a mut A,
    walker: Walker<T>,
}

impl<'a, A: PageTableAlloc, T: TranslatePhys> Mapper<'a, A, T> {
    /// # Safety
    ///
    /// The caller must guarantee that the provided table is correctly structured and that
    /// `translator` provides correct virtual page numbers for any queried physical pages.
    pub unsafe fn new(root_pt: &'a mut PageTable, alloc: &'a mut A, translator: T) -> Self {
        Self {
            root_pt,
            alloc,
            walker: unsafe { Walker::new(translator) },
        }
    }

    pub fn map(
        &mut self,
        virt: VirtPageNum,
        phys: PhysPageNum,
        perms: PageTablePerms,
    ) -> Result<(), MapError> {
        let mut pt = unsafe {
            self.walker.next_table_or_create(
                self.root_pt,
                virt.pt_index(PT_LEVEL_COUNT - 1),
                self.alloc,
                perms,
            )?
        };

        for level in (1..PT_LEVEL_COUNT - 1).rev() {
            pt = unsafe {
                self.walker
                    .next_table_or_create(pt, virt.pt_index(level), self.alloc, perms)?
            };
        }

        let target_entry = &mut pt[virt.pt_index(0)];
        if target_entry.flags().has_present() {
            return Err(MapError::EntryExists);
        }

        *target_entry = PageTableEntry::new(phys, flags_from_perms(perms));

        Ok(())
    }
}

fn flags_from_perms(perms: PageTablePerms) -> PageTableFlags {
    let mut flags = PageTableFlags::common();
    flags.apply_perms(perms);
    flags
}
