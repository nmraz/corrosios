use spin_once::Once;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END, PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};
use crate::arch::mmu::{flush_kernel_tlb, flush_kernel_tlb_page, kernel_pt_root};
use crate::kimage;
use crate::mm::types::{PageTablePerms, PhysFrameNum};

use super::aspace::{AddrSpace, AddrSpaceOps, TlbFlush};

/// Initializes the (higher-half) kernel address space.
///
/// # Panics
///
/// Panics if this function is called more than once.
pub fn init() {
    let aspace = unsafe {
        AddrSpace::new(KERNEL_ASPACE_BASE..KERNEL_ASPACE_END, KernelAddrSpaceOps)
            .expect("failed to create kernel address space")
    };

    let root_slice = aspace.root_slice();

    aspace
        .create_subslice(
            &root_slice,
            "physmap",
            Some(PHYS_MAP_BASE),
            PHYS_MAP_MAX_PAGES,
        )
        .expect("failed to reserve physmap virtual address space");

    aspace
        .create_subslice(
            &root_slice,
            "kimage",
            Some(kimage::virt_base()),
            kimage::total_pages(),
        )
        .expect("failed to reserve kernel image virtual address space");

    KERNEL_ASPACE.init(aspace);
}

/// Retrieves the global kernel address space.
///
/// # Panics
///
/// Panics if [`init`] has not yet been called.
pub fn get() -> &'static AddrSpace<impl AddrSpaceOps> {
    KERNEL_ASPACE
        .get()
        .expect("kernel address space not initialized")
}

struct KernelAddrSpaceOps;

unsafe impl AddrSpaceOps for KernelAddrSpaceOps {
    fn root_pt(&self) -> PhysFrameNum {
        kernel_pt_root()
    }

    fn flush(&self, request: TlbFlush<'_>) {
        match request {
            TlbFlush::Specific(pages) => {
                for &vpn in pages {
                    flush_kernel_tlb_page(vpn);
                }
            }
            TlbFlush::All => flush_kernel_tlb(),
        }
    }

    fn base_perms(&self) -> PageTablePerms {
        PageTablePerms::empty()
    }
}

static KERNEL_ASPACE: Once<AddrSpace<KernelAddrSpaceOps>> = Once::new();
