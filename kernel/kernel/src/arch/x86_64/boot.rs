use super::gdt::GDT_SIZE;

// Work around rustc bug that treats this constant as unused.
#[allow(dead_code)]
const KERNEL_OFFSET: u64 = 0xffffffff80000000;

global_asm!(include_str!("boot.s"), GDT_SIZE = const GDT_SIZE, KERNEL_OFFSET = const KERNEL_OFFSET);
