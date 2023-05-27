use core::arch::asm;
use core::cell::UnsafeCell;
use core::slice;

use bitflags::bitflags;
use log::trace;

use crate::arch::x86_64::x64_cpu::write_pat;
use crate::kimage;
use crate::mm::physmap::pfn_to_physmap;
use crate::mm::pmm;
use crate::mm::types::{CacheMode, PageTablePerms, PhysFrameNum, VirtAddr, VirtPageNum};
use crate::sync::irq::IrqDisabled;

use super::x64_cpu::{
    read_cr0, read_cr3, read_cr4, read_ia32_efer, read_mtrr_def_type, wbinvd, write_cr0, write_cr3,
    write_cr4, write_ia32_efer, write_mtrr_def_type, Cr0, Cr4, Ia32Efer,
};

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

pub const PT_LEVEL_COUNT: usize = 4;

pub const PT_LEVEL_SHIFT: usize = 9;
pub const PT_ENTRY_COUNT: usize = 1 << PT_LEVEL_SHIFT;
pub const PT_LEVEL_MASK: usize = PT_ENTRY_COUNT - 1;

const MTRR_DEF_TYPE_E: u64 = 1 << 11;
const MTRR_DEF_TYPE_TYPE_MASK: u64 = 0xff;

const MEM_TYPE_UC: u64 = 0;
const MEM_TYPE_WC: u64 = 1;
const MEM_TYPE_WT: u64 = 4;
const MEM_TYPE_WB: u64 = 6;
const MEM_TYPE_UC_WEAK: u64 = 6;

// We use the hardware (boot-up) defaults for most of the PAT entries, but change one to support
// WC.
const PA0_VAL: u64 = MEM_TYPE_WB; // Default
const PA1_VAL: u64 = MEM_TYPE_WT; // Default
const PA2_VAL: u64 = MEM_TYPE_UC_WEAK; // Default
const PA3_VAL: u64 = MEM_TYPE_UC; // Default
const PA4_VAL: u64 = MEM_TYPE_WB; // Default
const PA5_VAL: u64 = MEM_TYPE_WT; // Default
const PA6_VAL: u64 = MEM_TYPE_UC_WEAK; // Default
const PA7_VAL: u64 = MEM_TYPE_WC; // Weakened from default UC

// Keep these in sync with the `PA` values above!

// This should always be 0 so we have a safe default if someone mapping a page ignores the PAT bits.
const PAT_SELECTOR_WB: u64 = 0;
const PAT_SELECTOR_WT: u64 = 1;
const PAT_SELECTOR_UC: u64 = 3;
const PAT_SELECTOR_WC: u64 = 7;

const PT_RANGE: usize = 1 << (PT_LEVEL_SHIFT + PAGE_SHIFT);
const MB: usize = 0x100000;
const PADDR_MASK: u64 = (1u64 << 52) - 1;

// Note: keep in sync with linker script and early mapping in `boot.s`
const KERNEL_MAX: usize = 8 * MB;
const KERNEL_PT_COUNT: usize = KERNEL_MAX / PT_RANGE;

pub(super) static KERNEL_PML4: PageTableSpace = PageTableSpace::NEW;
pub(super) static KERNEL_PDPT: PageTableSpace = PageTableSpace::NEW;
pub(super) static KERNEL_PD: PageTableSpace = PageTableSpace::NEW;
pub(super) static KERNEL_PTS: [PageTableSpace; KERNEL_PT_COUNT] =
    [PageTableSpace::NEW; KERNEL_PT_COUNT];

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

#[repr(C, align(0x1000))]
pub struct PageTableSpace {
    entries: UnsafeCell<[PageTableEntry; PT_ENTRY_COUNT]>,
}

impl PageTableSpace {
    #[allow(clippy::declare_interior_mutable_const)]
    pub const NEW: Self = Self::new();

    pub const fn new() -> Self {
        Self {
            entries: UnsafeCell::new([make_empty_pte(); PT_ENTRY_COUNT]),
        }
    }
}

// Safety: this structure exists only to reserve BSS space for page tables, all accesses require
// unsafe code anyway.
unsafe impl Sync for PageTableSpace {}

bitflags! {
    struct X86PageTableFlags: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USER_MODE = 1 << 2;
        const GLOBAL = 1 << 8;

        const PERMS_MASK = Self::WRITABLE.bits() | Self::USER_MODE.bits() | Self::NO_EXEC.bits();

        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        const LARGE = 1 << 7;

        const NO_EXEC = 1 << 63;
    }
}

/// Performs early architecture-specific MMU initialization.
///
/// On x64, this currently:
/// * Enables global pages.
/// * Initializes the PAT so that caching modes can be safely used with the page table API later.
///
/// # Safety
///
/// This function should only be called once on the BSP.
pub unsafe fn init_early(_irq_disabled: &IrqDisabled) {
    init_mmu_regs();
    init_pat();
}

/// Returns the physical frame of the kernel root page table.
pub fn kernel_pt_root() -> PhysFrameNum {
    kimage::pfn_from_kernel_vpn(VirtAddr::from_ptr(&KERNEL_PML4).containing_page())
}

/// Performs final architecture-specific initialization of the kernel address space.
///
/// On x64,
///
/// # Safety
///
/// * This function should only be called once on the BSP, when the kernel page table is not being
///   used anywhere else.
/// * This function assumes that the frame allocator and physmap are initialized.
pub unsafe fn finish_init_kernel_pt() {
    // Safety: function contract
    let kernel_pml4 = unsafe { &mut *KERNEL_PML4.entries.get() };

    // Make sure that at least all the kernel PDPT pointers are initialized. This ensures that low
    // address spaces will always have a consistent view of the kernel address space later.
    for pte in &mut kernel_pml4[PT_ENTRY_COUNT / 2..] {
        if !pte_is_present(*pte, 3) {
            let next_table = pmm::allocate(0).expect("failed to allocate kernel-space PDPT");
            unsafe {
                pfn_to_physmap(next_table)
                    .addr()
                    .as_mut_ptr::<PageTableSpace>()
                    .write_bytes(0, 1)
            }

            *pte = make_intermediate_pte(3, next_table);
        }
    }
}

/// Queries whether the specified page table in kernel space can safely be freed back to the PMM
/// if necessary.
///
/// # Safety
///
/// The caller must guarantee that they will never call this function on tables mapping the kernel
/// image or the physmap, which should never be unmapped anyway.
pub unsafe fn can_cull_kernel_pt(_pt: PhysFrameNum, level: usize) -> bool {
    // We always keep all of the kernel PDPTs mapped, to make sure our shadowing into low address
    // spaces is consistent. Note that we also depend on the fact that the kernel image and physmap
    // will never be unmapped, so we don't have to bother checking whether we got a table that maps
    // them.
    level < 2
}

/// Prepares a new low root page table for use.
///
/// On x64, this shadows the kernel mappings into the upper half of `pt`.
///
/// # Safety
///
/// The caller must guarantee that the top-level kernel page table is not being modified
/// concurrently and will not be modified in the future.
pub unsafe fn prepare_low_pt_root(pt: &mut [PageTableEntry]) {
    // Safety: as per the function contract, nobody should be mutating this any longer.
    let kernel_pml4 = unsafe {
        slice::from_raw_parts(
            &KERNEL_PML4 as *const _ as *const PageTableEntry,
            PT_ENTRY_COUNT,
        )
    };

    pt[PT_ENTRY_COUNT / 2..].copy_from_slice(&kernel_pml4[PT_ENTRY_COUNT / 2..]);
}

/// Sets the root page table for the low half of the address space to `pt`.
///
/// If `pt` is `None`, the lower half of memory will be unmapped entirely, leaving only the high
/// kernel memory.
///
/// # Safety
///
/// This function is wildly unsafe, as it replaces the entire lower-half address space with a
/// different one. The caller must ensure that all accesses to low memory are made in accordance
/// with the new address space after the switch.
pub unsafe fn set_low_root_pt(pt: Option<PhysFrameNum>) {
    let pt = pt.unwrap_or_else(kernel_pt_root);
    unsafe {
        write_cr3(pt.addr().as_u64());
    }
}

/// Flushes the specified page from the kernel TLB.
pub fn flush_kernel_tlb_page(vpn: VirtPageNum) {
    trace!("flushing TLB page {vpn}");
    unsafe {
        asm!("invlpg [{}]", in(reg) vpn.addr().as_usize());
    }
}

/// Flushes the entire kernel TLB.
pub fn flush_kernel_tlb() {
    trace!("flushing kernel TLB");

    unsafe {
        let cr4 = read_cr4();
        assert!(
            cr4.contains(Cr4::PGE),
            "global pages should be enabled for kernel TLB flushes to work"
        );

        // See ISDM 3A, section 4.10.4.1
        write_cr4(cr4 & !Cr4::PGE);
        write_cr4(cr4);
    }
}

/// Flushes the specified page from the lower-half TLB.
pub fn flush_low_tlb_page(vpn: VirtPageNum) {
    flush_kernel_tlb_page(vpn);
}

/// Flushes the entire lower-half TLB.
pub fn flush_low_tlb() {
    trace!("flushing low TLB");
    // We currently don't use PCIDs at all.
    unsafe {
        write_cr3(read_cr3());
    }
}

/// Queries whether the processor supports large pages at level `level` of the page table hierarchy.
pub fn supports_page_size(level: usize) -> bool {
    matches!(level, 0 | 1)
}

/// Creates an empty (non-present) PTE.
pub const fn make_empty_pte() -> PageTableEntry {
    PageTableEntry(0)
}

/// Creates leaf a PTE mapping `frame` with permissions `perms` for use with the specified page
/// table level.
pub fn make_terminal_pte(
    level: usize,
    frame: PhysFrameNum,
    perms: PageTablePerms,
    cache_mode: CacheMode,
) -> PageTableEntry {
    let mut x86_flags = X86PageTableFlags::PRESENT | flags_from_perms(perms);

    x86_flags.set(X86PageTableFlags::LARGE, level > 0);

    PageTableEntry(
        frame.addr().as_u64()
            | x86_flags.bits()
            | pat_selector_to_pte_bits(pat_selector_for_cache_mode(cache_mode)),
    )
}

/// Creates a PTE referring to a lower-level page table `next_table` for use with the specified page
/// table level.
pub fn make_intermediate_pte(_level: usize, next_table: PhysFrameNum) -> PageTableEntry {
    let x86_flags =
        X86PageTableFlags::PRESENT | X86PageTableFlags::WRITABLE | X86PageTableFlags::USER_MODE;
    PageTableEntry(next_table.addr().as_u64() | x86_flags.bits())
}

pub fn update_pte_perms(
    pte: PageTableEntry,
    _level: usize,
    perms: PageTablePerms,
) -> PageTableEntry {
    PageTableEntry((pte.0 & !X86PageTableFlags::PERMS_MASK.bits()) | flags_from_perms(perms).bits())
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

fn init_mmu_regs() {
    unsafe {
        let mut cr0 = read_cr0();
        cr0 &= !(Cr0::CD | Cr0::NW);
        cr0 |= Cr0::WP;
        write_cr0(cr0);

        // Global pages will be enabled once the PAT is set up.
        let cr4 = read_cr4();
        write_cr4(cr4 & !Cr4::PGE);

        write_ia32_efer(read_ia32_efer() | Ia32Efer::NXE);
    }
}

fn init_pat() {
    // See ISDM 3A, section 11.12.4 and 11.11.8 on recommended procedure here. We probably don't
    // need a lot of the MTRR-related stuff, but keep it in just in case.
    unsafe {
        // 4. Enter the no-fill cache mode
        let cr0 = read_cr0();
        write_cr0(cr0 | Cr0::CD);

        // 5. Flush all caches with `wbinvd`
        wbinvd();

        // 6-7. Flush TLB
        // We intentionally use this sequence since global pages are not enabled yet (we do that
        // later, precisely so we can avoid repeated toggling of `PGE`).
        write_cr3(read_cr3());

        // 8. Disable all MTRRs by clearing the `E` flag in `MTRR_DEF_TYPE`
        let mut mtrr_def_type = read_mtrr_def_type();
        write_mtrr_def_type(mtrr_def_type & !MTRR_DEF_TYPE_E);

        // 9. Update the MTRRs and PAT

        write_pat(
            PA0_VAL
                | (PA1_VAL << 8)
                | (PA2_VAL << 16)
                | (PA3_VAL << 24)
                | (PA4_VAL << 32)
                | (PA5_VAL << 40)
                | (PA6_VAL << 48)
                | (PA7_VAL << 56),
        );

        // Override the default memory type to UC for consistency, all of our page tables should be
        // mapping WB (PAT index 0) by default anyway.
        mtrr_def_type = (mtrr_def_type & !MTRR_DEF_TYPE_TYPE_MASK) | MEM_TYPE_UC;

        // 10. Re-enable MTRRs
        write_mtrr_def_type(mtrr_def_type);

        // 11. Flush caches and TLB once more
        wbinvd();
        write_cr3(read_cr3());

        // 12. Restore normal cache operation
        write_cr0(cr0);

        // Now that the PAT is set up, enable global pages so we can start using them.
        write_cr4(read_cr4() | Cr4::PGE);
    }
}

fn flags_from_perms(perms: PageTablePerms) -> X86PageTableFlags {
    let mut x86_flags = X86PageTableFlags::empty();

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
    x86_flags.set(
        X86PageTableFlags::GLOBAL,
        perms.contains(PageTablePerms::GLOBAL),
    );

    x86_flags
}

fn pat_selector_for_cache_mode(cache_mode: CacheMode) -> u64 {
    match cache_mode {
        CacheMode::Cached => PAT_SELECTOR_WB,
        CacheMode::WriteThrough => PAT_SELECTOR_WT,
        CacheMode::WriteCombining => PAT_SELECTOR_WC,
        CacheMode::Uncached => PAT_SELECTOR_UC,
    }
}

fn pat_selector_to_pte_bits(pat_selector: u64) -> u64 {
    // Split the 3 bits of the pat selector across the `PWT`, `PCD` and `PAT` bits.
    ((pat_selector & 0b001) << 3) | ((pat_selector & 0b010) << 4) | ((pat_selector & 0b100) << 7)
}
