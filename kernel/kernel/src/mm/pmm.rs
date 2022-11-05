use core::alloc::Layout;
use core::{array, cmp, ptr, slice};

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use itertools::Itertools;
use log::debug;

use bitmap::BorrowedBitmapMut;
use num_utils::{div_ceil, log2};

use crate::arch::mmu::PAGE_SIZE;
use crate::mm::physmap::{paddr_to_physmap, physmap_to_pfn};
use crate::mm::types::PhysFrameNum;
use crate::mm::utils::display_byte_size;
use crate::sync::irq::IrqDisabled;
use crate::sync::SpinLock;

use super::early::BootHeap;
use super::physmap::pfn_to_physmap;
use super::types::VirtAddr;

const ORDER_COUNT: usize = 16;

static PHYS_MANAGER: SpinLock<Option<PhysManager>> = SpinLock::new(None);

/// Initializes the physical memory manager (PMM) with space for tracking physical frames up to
/// `max_pfn`.
///
/// `bootheap` will be used for any necessary metadata allocations.
///
/// # Safety
///
/// * `bootheap` must point to safely usable free memory
///
/// # Panics
///
/// Panics if this function is called more than once.
pub unsafe fn init(max_pfn: PhysFrameNum, bootheap: &mut BootHeap, irq_disabled: &IrqDisabled) {
    PHYS_MANAGER.with_noirq(irq_disabled, |manager_ref| {
        assert!(manager_ref.is_none(), "pmm already initialized");

        debug!("reserving bitmaps up to frame {}", max_pfn);
        let manager = PhysManager::new(max_pfn, bootheap);
        *manager_ref = Some(manager);
    });
}

/// Allocates a block of physical pages of size and alignment `2 ** order`, returning the base
/// of the allocated block, or `None` if not enough memory is available.
pub fn allocate(order: usize) -> Option<PhysFrameNum> {
    with(|pmm| pmm.allocate(order))
}

/// Frees a block of physical pages previously allocated by [`allocate`].
///
/// # Safety
///
/// * `pfn` must have been obtained by a previous successfull call to [`allocate`] with `order`
/// * The pages should no longer be accessed after this function returns
pub unsafe fn deallocate(pfn: PhysFrameNum, order: usize) {
    with(|pmm| unsafe { pmm.deallocate(pfn, order) })
}

/// Marks the range `start..end` as free in the PMM.
///
/// # Safety
///
/// The reported range should contain free memory that can safely be repurposed, and should not
/// overlap any ranges added to the PMM by previous calls to `add_free_range`. The range should also
/// be present in the physmap.
pub unsafe fn add_free_range(start: PhysFrameNum, end: PhysFrameNum, irq_disabled: &IrqDisabled) {
    debug!("adding free range {}-{}", start, end);
    with_noirq(irq_disabled, |pmm| unsafe {
        pmm.add_free_range(start, end)
    })
}

pub fn dump_usage() {
    with(|pmm| pmm.dump_usage());
}

fn with_noirq<R>(irq_disabled: &IrqDisabled, f: impl FnOnce(&mut PhysManager) -> R) -> R {
    PHYS_MANAGER.with_noirq(irq_disabled, |pmm| {
        f(pmm.as_mut().expect("pmm not initialized"))
    })
}
fn with<R>(f: impl FnOnce(&mut PhysManager) -> R) -> R {
    PHYS_MANAGER.with(|pmm, _| f(pmm.as_mut().expect("pmm not initialized")))
}

struct PhysManager {
    total_pages: usize,
    levels: [BuddyLevel; ORDER_COUNT],
}

impl PhysManager {
    fn new(max_pfn: PhysFrameNum, bootheap: &mut BootHeap) -> Self {
        let levels = array::from_fn(|order| {
            // Note: the bitmap in each level tracks *pairs* of blocks on that level
            let splitmap_bits = div_ceil(max_pfn.as_usize(), 1 << (order + 1));
            let splitmap_bytes = bitmap::bytes_required(splitmap_bits);

            let splitmap_ptr: *mut u8 = paddr_to_physmap(bootheap.alloc_phys(
                Layout::from_size_align(splitmap_bytes, 1).expect("buddy bitmap too large"),
            ))
            .as_mut_ptr();

            let splitmap_slice = unsafe {
                ptr::write_bytes(splitmap_ptr, 0, splitmap_bytes);
                slice::from_raw_parts_mut(splitmap_ptr, splitmap_bytes)
            };

            BuddyLevel {
                free_list: LinkedList::new(FreePageAdapter::new()),
                free_blocks: 0,
                splitmap: BorrowedBitmapMut::new(splitmap_slice),
            }
        });

        Self {
            total_pages: 0,
            levels,
        }
    }

    fn allocate(&mut self, order: usize) -> Option<PhysFrameNum> {
        if order >= ORDER_COUNT {
            return None;
        }

        let mut pfn = None;
        let mut found_order = order;
        while found_order < ORDER_COUNT {
            if let Some(found) = self.levels[found_order].pop_free() {
                pfn = Some(found);
                break;
            }
            found_order += 1;
        }

        let pfn = pfn?;
        self.toggle_parent_split(pfn, found_order);

        // If we've found a block of a larger order, split it all the way down to the desired order.
        for cur_order in order..found_order {
            // Note: this will always set the bit, as we started with a larger (unsplit) block
            self.toggle_parent_split(pfn, cur_order);
            unsafe {
                self.levels[cur_order].push_free(buddy_of(pfn, cur_order));
            }
        }

        Some(pfn)
    }

    unsafe fn deallocate(&mut self, mut pfn: PhysFrameNum, mut order: usize) {
        assert!(pfn.as_usize() & ((1 << order) - 1) == 0);

        while order < ORDER_COUNT - 1 {
            self.toggle_parent_split(pfn, order);
            if self.is_parent_split(pfn, order) {
                // Our parent is now split, meaning that our buddy is allocated, so we can't merge.
                break;
            }

            // Merge with our buddy and keep checking higher orders
            pfn = parent_of(pfn, order);
            order += 1;
        }

        unsafe {
            self.levels[order].push_free(pfn);
        }
    }

    fn dump_usage(&self) {
        let free_pages = self.free_pages();
        let used_pages = self.total_pages - free_pages;

        debug!(
            "{} pages ({}) total, {} pages ({}) in use, {} pages ({}) free",
            self.total_pages,
            display_byte_size(self.total_pages * PAGE_SIZE),
            used_pages,
            display_byte_size(used_pages * PAGE_SIZE),
            free_pages,
            display_byte_size(free_pages * PAGE_SIZE)
        );
        debug!("free blocks by order:");
        debug!(
            "order: {}",
            (0..ORDER_COUNT).format_with(" ", |order, f| f(&format_args!("{:4}", order)))
        );
        debug!(
            "count: {}",
            (0..ORDER_COUNT)
                .map(|order| self.levels[order].free_blocks)
                .format_with(" ", |free, f| f(&format_args!("{:4}", free)))
        );
    }

    unsafe fn add_free_range(&mut self, mut start: PhysFrameNum, end: PhysFrameNum) {
        let size = end - start;

        while start < end {
            let remaining_order = log2(end - start);
            let alignment_order = start.as_usize().trailing_zeros() as usize;

            let order = cmp::min(alignment_order, remaining_order);
            let order = cmp::min(order, ORDER_COUNT - 1);

            unsafe {
                self.deallocate(start, order);
            }

            start += 1 << order;
        }

        self.total_pages += size;
    }

    fn free_pages(&self) -> usize {
        self.levels
            .iter()
            .enumerate()
            .map(|(order, level)| level.free_blocks << order)
            .sum()
    }

    fn toggle_parent_split(&mut self, pfn: PhysFrameNum, order: usize) {
        let index = splitmap_index(pfn, order);
        self.levels[order].toggle_parent_split(index);
    }

    fn is_parent_split(&self, pfn: PhysFrameNum, order: usize) -> bool {
        let index = splitmap_index(pfn, order);
        self.levels[order].is_parent_split(index)
    }
}

fn splitmap_index(pfn: PhysFrameNum, order: usize) -> usize {
    // Note: we take `order + 1` as every splitmap bit tracks *pairs* of blocks of the given order
    pfn.as_usize() >> (order + 1)
}

fn buddy_of(pfn: PhysFrameNum, order: usize) -> PhysFrameNum {
    PhysFrameNum::new(pfn.as_usize() ^ (1 << order))
}

fn parent_of(pfn: PhysFrameNum, order: usize) -> PhysFrameNum {
    PhysFrameNum::new(pfn.as_usize() & !(1usize << order))
}

struct FreePage {
    link: LinkedListLink,
}

intrusive_adapter!(FreePageAdapter = UnsafeRef<FreePage>: FreePage { link: LinkedListLink });

struct BuddyLevel {
    free_list: LinkedList<FreePageAdapter>,
    free_blocks: usize,
    splitmap: BorrowedBitmapMut<'static>,
}

impl BuddyLevel {
    fn toggle_parent_split(&mut self, index: usize) {
        self.splitmap.toggle(index);
    }

    fn is_parent_split(&self, index: usize) -> bool {
        self.splitmap.get(index)
    }

    unsafe fn push_free(&mut self, pfn: PhysFrameNum) {
        let link = unsafe {
            let ptr = free_link_from_pfn(pfn);
            ptr.write(FreePage {
                link: LinkedListLink::new(),
            });
            UnsafeRef::from_raw(ptr)
        };

        self.free_list.push_front(link);
        self.free_blocks += 1;
    }

    fn pop_free(&mut self) -> Option<PhysFrameNum> {
        let link = self.free_list.pop_front()?;
        self.free_blocks -= 1;
        Some(pfn_from_free_link(UnsafeRef::into_raw(link)))
    }
}

fn free_link_from_pfn(pfn: PhysFrameNum) -> *mut FreePage {
    pfn_to_physmap(pfn).addr().as_mut_ptr()
}

fn pfn_from_free_link(link: *const FreePage) -> PhysFrameNum {
    let vaddr = VirtAddr::from_ptr(link);
    assert_eq!(vaddr.page_offset(), 0);
    physmap_to_pfn(vaddr.containing_page())
}
