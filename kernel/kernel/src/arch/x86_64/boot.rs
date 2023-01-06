use core::arch::global_asm;

use crate::kernel_main;

use super::mmu::{
    KERNEL_PD, KERNEL_PDPT, KERNEL_PML4, KERNEL_PTS, PAGE_SHIFT, PAGE_SIZE, PT_ENTRY_COUNT,
    PT_LEVEL_SHIFT,
};

global_asm!(include_str!("boot.s"),
    PAGE_SHIFT = const PAGE_SHIFT,
    PT_LEVEL_SHIFT = const PT_LEVEL_SHIFT,
    PAGE_SIZE = const PAGE_SIZE,
    PT_ENTRY_COUNT = const PT_ENTRY_COUNT,

    KERNEL_PML4 = sym KERNEL_PML4,
    KERNEL_PDPT = sym KERNEL_PDPT,
    KERNEL_PD = sym KERNEL_PD,
    KERNEL_PTS = sym KERNEL_PTS,

    kernel_main = sym kernel_main,
);
