use super::gdt::{GDT_SIZE, KERNEL_CS_SELECTOR};
use super::mmu::{PAGE_SHIFT, PAGE_SIZE, PT_ENTRY_COUNT, PT_LEVEL_SHIFT};

// Work around rustc bug that treats this constant as unused.
#[allow(dead_code)]
const KERNEL_OFFSET: u64 = 0xffffffff80000000;

global_asm!(include_str!("boot.s"),
            GDT_SIZE = const GDT_SIZE,
            KERNEL_OFFSET = const KERNEL_OFFSET,
            PAGE_SHIFT = const PAGE_SHIFT,
            PT_LEVEL_SHIFT = const PT_LEVEL_SHIFT,
            PAGE_SIZE = const PAGE_SIZE,
            PT_ENTRY_COUNT = const PT_ENTRY_COUNT,
            KERNEL_CS_SELECTOR = const KERNEL_CS_SELECTOR);
