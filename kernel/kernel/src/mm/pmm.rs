use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mmu::PAGE_SIZE;
use crate::mm::bootheap::BootHeap;
use crate::mm::types::PhysFrameNum;

pub unsafe fn init(mem_map: &[MemoryRange]) {
    println!("\nfirmware memory map:");
    for range in mem_map {
        display_range(range);
    }

    println!();

    let kernel_base = PhysFrameNum::new(0x104);
    let mut bootheap = BootHeap::new(
        mem_map,
        &[
            kernel_base..kernel_base + 0x100,
            kernel_base + 0x200..kernel_base + 0x204,
            kernel_base + 0x208..PhysFrameNum::new(0x80a),
            PhysFrameNum::new(0x80b)..PhysFrameNum::new(0x80d),
            PhysFrameNum::new(0x80e)..PhysFrameNum::new(0x80f),
        ],
    );

    todo!()
}

fn display_range(range: &MemoryRange) {
    let kind = match range.kind {
        MemoryKind::RESERVED => "reserved",
        MemoryKind::USABLE => "usable",
        MemoryKind::FIRMWARE => "firmware",
        MemoryKind::ACPI_TABLES => "ACPI tables",
        MemoryKind::UNUSABLE => "unusable",
        _ => "other",
    };

    println!(
        "{:#012x}-{:#012x}: {}",
        range.start_page * PAGE_SIZE,
        (range.start_page + range.page_count) * PAGE_SIZE,
        kind
    );
}
