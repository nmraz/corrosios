use core::alloc::Layout;
use core::ops::Range;
use core::{array, cmp, mem, ptr, slice};

use arrayvec::ArrayVec;
use bootinfo::item::{MemoryKind, MemoryRange};

use crate::mm::bootheap::BootHeap;
use crate::mm::physmap::paddr_to_physmap;
use crate::mm::types::PhysFrameNum;
use crate::mm::utils::{self, div_ceil, prev_power_of_two};
use crate::sync::SpinLock;

use super::physmap::pfn_to_physmap;

const ORDER_COUNT: usize = 15;

static PHYS_MANAGER: SpinLock<Option<PhysManager>> = SpinLock::new(None);

pub struct PhysManager {
    levels: [BuddyLevel; ORDER_COUNT],
}

impl PhysManager {
    pub fn allocate(&mut self, order: usize) -> Option<PhysFrameNum> {
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
        let mut cur_pfn = pfn;
        for cur_order in order..found_order {
            // Note: this will always set the bit, as we started with a larger (unsplit) block
            self.toggle_parent_split(cur_pfn, cur_order);
            unsafe {
                self.levels[cur_order].push_free(buddy(cur_pfn, cur_order));
            }
            cur_pfn = parent(cur_pfn, cur_order);
        }

        Some(pfn)
    }

    pub unsafe fn deallocate(&mut self, mut pfn: PhysFrameNum, mut order: usize) {
        assert!(pfn.as_usize() & ((1 << order) - 1) == 0);

        while order < ORDER_COUNT - 1 {
            self.toggle_parent_split(pfn, order);
            if self.is_parent_split(pfn, order) {
                // Our parent is now split, meaning that our buddy is allocated, so we can't merge.
                break;
            }

            // Merge with our buddy and keep checking higher orders
            pfn = parent(pfn, order);
            order += 1;
        }

        unsafe {
            self.levels[order].push_free(pfn);
        }
    }

    pub fn dump_usage(&self) {
        // println!("free pages: {}", self.free_pages());
        // println!("free blocks by order:");
        // for order in 0..ORDER_LIMIT {
        //     println!("{}: {}", order, self.levels[order].free_blocks);
        // }
    }
}

/// Initializes the physical memory manager (PMM) for all usable ranges in `mem_map`, carving out
/// holes as specified in `reserved_ranges`. `bootheap` will be used for any necessary metadata
/// allocations, the page range covered by it will also be marked as reserved when the manager is
/// initialized.
///
/// # Safety
///
/// * `mem_map` must contain non-overlapping entries
/// * Entries marked as usable in `mem_map` must point to valid, usable memory
/// * Frames in entries marked as usable in `mem_map` should no longer be accessed directly, as they
///   are now owned by the PMM
pub unsafe fn init(
    mem_map: &[MemoryRange],
    reserved_ranges: &[Range<PhysFrameNum>],
    mut bootheap: BootHeap,
) {
    let mut manager_ref = PHYS_MANAGER.lock();
    assert!(manager_ref.is_none(), "pmm already initialized");

    let max_pfn = highest_usable_frame(mem_map);
    println!("pmm: reserving bitmaps up to frame {}", max_pfn);
    let mut manager = PhysManager::new(max_pfn, &mut bootheap);

    let bootheap_used_range = bootheap.used_range();
    println!(
        "pmm: final bootheap usage: {}-{} (~{}K)",
        bootheap_used_range.start,
        bootheap_used_range.end,
        div_ceil(bootheap_used_range.end - bootheap_used_range.start, 1024)
    );

    let bootheap_used_frames = bootheap_used_range.start.containing_frame()
        ..bootheap_used_range.end.containing_tail_frame();

    let reserved_ranges = {
        let mut final_reserved_ranges: ArrayVec<_, 5> = ArrayVec::new();
        final_reserved_ranges.extend(reserved_ranges.iter().cloned());
        final_reserved_ranges.push(bootheap_used_frames);
        final_reserved_ranges.sort_unstable_by_key(|range| range.start);
        final_reserved_ranges
    };

    utils::iter_usable_ranges(mem_map, &reserved_ranges, |start, end| {
        println!("pmm: adding free range {}-{}", start, end);
        manager.add_free_range(start, end);
    });

    *manager_ref = Some(manager);
}

pub fn with<R>(f: impl FnOnce(&mut PhysManager) -> R) -> R {
    f(PHYS_MANAGER.lock().as_mut().expect("pmm not initialized"))
}

fn highest_usable_frame(mem_map: &[MemoryRange]) -> PhysFrameNum {
    mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE)
        .map(|range| PhysFrameNum::new(range.start_page) + range.page_count)
        .max()
        .expect("no usable memory")
}

impl PhysManager {
    fn new(max_pfn: PhysFrameNum, bootheap: &mut BootHeap) -> Self {
        let levels = array::from_fn(|order| {
            // Note: the bitmap in each level tracks *pairs* of blocks on that level
            let splitmap_bits = div_ceil(max_pfn.as_usize(), 1 << (order + 1));
            let splitmap_bytes = div_ceil(splitmap_bits, 8);

            let splitmap_ptr: *mut u8 = paddr_to_physmap(bootheap.alloc_phys(
                Layout::from_size_align(splitmap_bytes, 1).expect("buddy bitmap too large"),
            ))
            .as_mut_ptr();

            let splitmap = unsafe {
                ptr::write_bytes(splitmap_ptr, 0, splitmap_bytes);
                slice::from_raw_parts_mut(splitmap_ptr, splitmap_bytes)
            };

            BuddyLevel {
                free_list: None,
                free_blocks: 0,
                splitmap,
            }
        });

        Self { levels }
    }

    fn add_free_range(&mut self, mut start: PhysFrameNum, end: PhysFrameNum) {
        while start < end {
            let remaining_order = prev_power_of_two(end - start);
            let alignment_order = start.as_usize().trailing_zeros() as usize;

            let order = cmp::min(alignment_order, remaining_order);
            let order = cmp::min(order, ORDER_COUNT - 1);

            unsafe {
                self.deallocate(start, order);
            }

            start += 1 << order;
        }

        assert_eq!(start, end);
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

fn buddy(pfn: PhysFrameNum, order: usize) -> PhysFrameNum {
    PhysFrameNum::new(pfn.as_usize() ^ (1 << order))
}

fn parent(pfn: PhysFrameNum, order: usize) -> PhysFrameNum {
    PhysFrameNum::new(pfn.as_usize() & !(1usize << order))
}

struct FreeLink {
    prev: PhysFrameNum,
    next: PhysFrameNum,
}

struct BuddyLevel {
    free_list: Option<PhysFrameNum>,
    free_blocks: usize,
    splitmap: &'static mut [u8],
}

impl BuddyLevel {
    fn toggle_parent_split(&mut self, index: usize) {
        let byte = index / 8;
        let bit = index % 8;

        self.splitmap[byte] ^= 1 << bit;
    }

    fn is_parent_split(&self, index: usize) -> bool {
        let byte = index / 8;
        let bit = index % 8;

        (self.splitmap[byte] >> bit) & 1 != 0
    }

    fn pop_free(&mut self) -> Option<PhysFrameNum> {
        let head = self.free_list?;
        unsafe {
            self.detach_free(head);
        }
        Some(head)
    }

    unsafe fn detach_free(&mut self, pfn: PhysFrameNum) {
        let next = unsafe { detach_free_link(pfn) };
        self.free_blocks -= 1;
        if self.free_list == Some(pfn) {
            self.free_list = next;
        }
    }

    unsafe fn push_free(&mut self, pfn: PhysFrameNum) {
        unsafe {
            if let Some(head) = self.free_list {
                push_free_link(head, pfn);
            } else {
                init_free_link(pfn);
            }
        }
        self.free_blocks += 1;
        self.free_list = Some(pfn);
    }
}

unsafe fn init_free_link(pfn: PhysFrameNum) {
    let link = free_link_from_pfn(pfn);

    unsafe {
        link.write(FreeLink {
            prev: pfn,
            next: pfn,
        });
    }
}

unsafe fn push_free_link(head: PhysFrameNum, new_head: PhysFrameNum) {
    let head_link = free_link_from_pfn(head);
    let new_head_link = free_link_from_pfn(new_head);

    unsafe {
        let prev = (*head_link).prev;
        let prev_link = free_link_from_pfn(prev);

        (*prev_link).next = new_head;
        (*head_link).prev = new_head;

        new_head_link.write(FreeLink { prev, next: head });
    }
}

unsafe fn detach_free_link(pfn: PhysFrameNum) -> Option<PhysFrameNum> {
    let link = free_link_from_pfn(pfn);

    unsafe {
        let prev = (*link).prev;
        let next = (*link).next;

        let prev_link = free_link_from_pfn(prev);
        let next_link = free_link_from_pfn(next);

        (*prev_link).next = next;
        (*next_link).prev = prev;

        if next != pfn {
            Some(next)
        } else {
            None
        }
    }
}

fn free_link_from_pfn(pfn: PhysFrameNum) -> *mut FreeLink {
    pfn_to_physmap(pfn).addr().as_mut_ptr()
}
