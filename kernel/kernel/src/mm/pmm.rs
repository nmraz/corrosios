use bootinfo::item::{MemoryKind, MemoryRange};

use crate::arch::mmu::PAGE_SIZE;

pub unsafe fn init(mem_map: &[MemoryRange]) {
    println!("\nfirmware memory map:");
    for range in mem_map {
        display_range(range);
    }

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
