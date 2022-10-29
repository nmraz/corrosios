use core::arch::asm;
use core::sync::atomic::AtomicU64;

use bitflags::bitflags;

use crate::kimage;
use crate::mm::types::{PageTablePerms, PhysFrameNum, VirtAddr, VirtPageNum};

use super::x64_cpu::{read_cr3, write_cr3};

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const PT_LEVEL_COUNT: usize = 4;

pub const PT_LEVEL_SHIFT: usize = 9;
pub const PT_ENTRY_COUNT: usize = 1 << PT_LEVEL_SHIFT;
pub const PT_LEVEL_MASK: usize = PT_ENTRY_COUNT - 1;

const PT_RANGE: usize = 1 << (PT_LEVEL_SHIFT + PAGE_SHIFT);
const MB: usize = 0x100000;
const PADDR_MASK: u64 = (1u64 << 52) - 1;

// Note: keep in sync with linker script and early mapping in `boot.s`
const KERNEL_MAX: usize = 8 * MB;
const KERNEL_PT_COUNT: usize = KERNEL_MAX / PT_RANGE;

#[no_mangle]
static KERNEL_PML4: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PDPT: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PD: PageTableSpace = PageTableSpace::NEW;

#[no_mangle]
static KERNEL_PTS: [PageTableSpace; KERNEL_PT_COUNT] = [PageTableSpace::NEW; KERNEL_PT_COUNT];

/// Returns the physical frame of the kernel root page table.
pub fn kernel_pt_root() -> PhysFrameNum {
    kimage::pfn_from_kernel_vpn(VirtAddr::from_ptr(&KERNEL_PML4).containing_page())
}

/// Flushes the specified page from the kernel TLB.
pub fn flush_kernel_tlb_page(vpn: VirtPageNum) {
    unsafe {
        asm!("invlpg [{}]", in(reg) vpn.addr().as_usize());
    }
}

/// Flushes the entire kernel TLB.
pub fn flush_kernel_tlb() {
    unsafe {
        write_cr3(read_cr3());
    }
}

/// Queries whether the processor supports large pages at level `level` of the page table hierarchy.
pub fn supports_page_size(level: usize) -> bool {
    matches!(level, 0 | 1)
}

/// Creates an empty (non-present) PTE.
pub fn make_empty_pte() -> PageTableEntry {
    PageTableEntry(0)
}

/// Creates a PTE for use at the specified page table level with the specified permissions and
/// physical frame.
///
/// If `terminal` is true, the PTE will be set up as a pointer to a leaf entry mapping a physical
/// frame. Otherwise, the PTE will be set up to point to the next-level page table at `frame`.
pub fn make_pte(
    level: usize,
    terminal: bool,
    frame: PhysFrameNum,
    perms: PageTablePerms,
) -> PageTableEntry {
    let mut x86_flags = X86PageTableFlags::PRESENT;

    x86_flags.set(
        X86PageTableFlags::WRITABLE,
        perms.contains(PageTablePerms::WRITE),
    );
    x86_flags.set(
        X86PageTableFlags::USER_MODE,
        perms.contains(PageTablePerms::USER),
    );
    x86_flags.set(
        X86PageTableFlags::NO_EXEC,
        !perms.contains(PageTablePerms::EXECUTE),
    );

    x86_flags.set(X86PageTableFlags::LARGE, level > 0 && terminal);

    PageTableEntry(frame.addr().as_u64() | x86_flags.bits())
}

pub fn get_pte_frame(pte: PageTableEntry, _level: usize) -> PhysFrameNum {
    PhysFrameNum::new(((pte.0 & PADDR_MASK) >> PAGE_SHIFT) as usize)
}

pub fn pte_is_present(pte: PageTableEntry, _level: usize) -> bool {
    X86PageTableFlags::from_bits_truncate(pte.0).contains(X86PageTableFlags::PRESENT)
}

pub fn pte_is_terminal(pte: PageTableEntry, level: usize) -> bool {
    if level == 0 {
        true
    } else {
        X86PageTableFlags::from_bits_truncate(pte.0).contains(X86PageTableFlags::LARGE)
    }
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

#[repr(C, align(0x1000))]
pub struct PageTableSpace {
    entries: [AtomicU64; PT_ENTRY_COUNT],
}

impl PageTableSpace {
    #[allow(clippy::declare_interior_mutable_const)]
    pub const NEW: Self = Self::new();

    pub const fn new() -> Self {
        #[allow(clippy::declare_interior_mutable_const)]
        const INIT_ENTRY: AtomicU64 = AtomicU64::new(0);
        Self {
            entries: [INIT_ENTRY; PT_ENTRY_COUNT],
        }
    }
}

bitflags! {
    struct X86PageTableFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_MODE = 1 << 2;

        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const LARGE = 1 << 7;

        const NO_EXEC = 1 << 63;
    }
}
