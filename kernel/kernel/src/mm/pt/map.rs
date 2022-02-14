use crate::arch::mmu::{PageTable, PageTableEntry, PT_LEVEL_COUNT};
use crate::mm::types::{PageTablePerms, PhysPageNum, VirtPageNum};

use super::walk::Walker;
use super::{flags_from_perms, PageTableAlloc, PageTableAllocError, TranslatePhys};

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
