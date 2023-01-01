use log::debug;
use spin_once::Once;

use crate::arch::mm::{KERNEL_ASPACE_BASE, KERNEL_ASPACE_END, PHYS_MAP_BASE, PHYS_MAP_MAX_PAGES};
use crate::arch::mmu::{
    can_cull_kernel_pt, finish_init_kernel_pt, flush_kernel_tlb, flush_kernel_tlb_page,
    kernel_pt_root,
};
use crate::kimage;
use crate::mm::physmap::PhysmapPfnTranslator;
use crate::mm::pt::{MappingPointer, NoopGather, PageTable};
use crate::mm::types::{PageTablePerms, PhysFrameNum};

use super::aspace::{AddrSpace, AddrSpaceOps, TlbFlush};

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

/// Initializes the (higher-half) kernel address space.
///
/// # Panics
///
/// Panics if this function is called more than once.
pub(super) fn init() {
    let aspace = unsafe {
        AddrSpace::new(KERNEL_ASPACE_BASE..KERNEL_ASPACE_END, KernelAddrSpaceOps)
            .expect("failed to create kernel address space")
    };

    let root_slice = aspace.root_slice();

    aspace
        .create_subslice(
            root_slice,
            "physmap",
            Some(PHYS_MAP_BASE),
            PHYS_MAP_MAX_PAGES,
        )
        .expect("failed to reserve physmap virtual address space");

    aspace
        .create_subslice(
            root_slice,
            "kernel image",
            Some(kimage::virt_base()),
            kimage::total_pages(),
        )
        .expect("failed to reserve kernel image virtual address space");

    KERNEL_ASPACE.init(aspace);

    unsafe {
        finish_init_kernel_pt();
        protect_kimage();
    }
}

unsafe fn protect_kimage() {
    debug!("protecting kernel image");

    unsafe {
        let mut pt = PageTable::new(kernel_pt_root(), PhysmapPfnTranslator);

        pt.protect(
            &mut NoopGather,
            &mut MappingPointer::new(kimage::code_base(), kimage::code_pages()),
            PageTablePerms::EXECUTE | PageTablePerms::GLOBAL,
        )
        .expect("failed to protect kernel code");

        pt.protect(
            &mut NoopGather,
            &mut MappingPointer::new(kimage::rodata_base(), kimage::rodata_pages()),
            PageTablePerms::READ | PageTablePerms::GLOBAL,
        )
        .expect("failed to protect kernel rodata");

        pt.protect(
            &mut NoopGather,
            &mut MappingPointer::new(kimage::data_base(), kimage::data_pages()),
            PageTablePerms::READ | PageTablePerms::WRITE | PageTablePerms::GLOBAL,
        )
        .expect("failed to protect kernel data");

        flush_kernel_tlb();
    }
}

struct KernelAddrSpaceOps;

unsafe impl AddrSpaceOps for KernelAddrSpaceOps {
    fn root_pt(&self) -> PhysFrameNum {
        kernel_pt_root()
    }

    fn flush(&self, request: TlbFlush<'_>) {
        // TODO: full shootdown here
        match request {
            TlbFlush::Specific(pages) => {
                for &vpn in pages {
                    flush_kernel_tlb_page(vpn);
                }
            }
            TlbFlush::All => flush_kernel_tlb(),
        }
    }

    fn can_cull_pt(&self, pt: PhysFrameNum, level: usize) -> bool {
        // Safety: we don't even have a mapping covering the kernel image or physmap, and we are
        // careful not to allow outside accesses to the entire kernel address space.
        unsafe { can_cull_kernel_pt(pt, level) }
    }

    fn base_perms(&self) -> PageTablePerms {
        PageTablePerms::GLOBAL
    }
}

static KERNEL_ASPACE: Once<AddrSpace<KernelAddrSpaceOps>> = Once::new();
