use super::{PageTable, PAGE_SHIFT, PT_LEVEL_SHIFT};

const MB: usize = 0x100000;

const PT_RANGE: usize = 1 << (PT_LEVEL_SHIFT + PAGE_SHIFT);

// Note: keep in sync with linker script and early mapping in `boot.s`
const KERNEL_MAX: usize = 8 * MB;

const KERNEL_PT_COUNT: usize = KERNEL_MAX / PT_RANGE;

#[no_mangle]
static KERNEL_PML4: PageTable = PageTable::EMPTY;

#[no_mangle]
static KERNEL_PDPT: PageTable = PageTable::EMPTY;

#[no_mangle]
static KERNEL_PD: PageTable = PageTable::EMPTY;

#[no_mangle]
static KERNEL_PTS: [PageTable; KERNEL_PT_COUNT] = [PageTable::EMPTY; KERNEL_PT_COUNT];

pub fn kernel_pt_root() -> &'static PageTable {
    &KERNEL_PML4
}
