use core::ptr;

use crate::arch::mmu::{PageTable, PageTableEntry};
use crate::mm::types::PageTablePerms;

use super::{flags_from_perms, PageTableAlloc, PageTableAllocError, TranslatePhys};

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
