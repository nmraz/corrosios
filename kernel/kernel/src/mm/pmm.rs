use core::ops::Range;
use core::ptr::NonNull;

use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mmu::PAGE_SIZE;
use crate::arch::pmm::BOOTHEAP_BASE;
use crate::kimage;
use crate::mm::bootheap::BootHeap;
use crate::mm::types::PhysFrameNum;
use crate::sync::SpinLock;

use super::physmap::pfn_to_physmap;
use super::types::PhysAddr;

const MAX_ORDER: usize = 15;

static PHYS_MANAGER: SpinLock<Option<PhysManager>> = SpinLock::new(None);

pub unsafe fn init(mem_map: &[MemoryRange], bootheap: BootHeap) {
    let bootheap_used_range = bootheap.used_range();
    println!(
        "\nbootheap usage: {}K",
        (bootheap_used_range.end - bootheap_used_range.start) / 1024
    );

    todo!()
}

pub fn alloc_pages(order: usize) -> Option<PhysFrameNum> {
    PHYS_MANAGER
        .lock()
        .as_mut()
        .expect("PMM not initialized")
        .allocate(order)
}

pub unsafe fn dealloc_pages(pfn: PhysFrameNum, order: usize) {
    unsafe {
        PHYS_MANAGER
            .lock()
            .as_mut()
            .expect("PMM not initialized")
            .deallocate(pfn, order);
    }
}

fn highest_usable_frame(mem_map: &[MemoryRange]) -> PhysFrameNum {
    mem_map
        .iter()
        .filter(|range| range.kind == MemoryKind::USABLE)
        .map(|range| PhysFrameNum::new(range.start_page) + range.page_count)
        .max()
        .expect("no usable memory")
}

struct PhysManager {
    levels: [BuddyLevel; MAX_ORDER],
}

impl PhysManager {
    fn allocate(&mut self, order: usize) -> Option<PhysFrameNum> {
        if order >= MAX_ORDER {
            return None;
        }

        let mut pfn = None;
        let mut found_order = order;
        while found_order < MAX_ORDER {
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

    unsafe fn deallocate(&mut self, mut pfn: PhysFrameNum, mut order: usize) {
        while order < MAX_ORDER {
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
