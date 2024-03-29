use crate::mm::types::{PhysAddr, PhysFrameNum, VirtAddr, VirtPageNum};

static mut KERNEL_PHYS: PhysFrameNum = PhysFrameNum::new(0);

extern "C" {
    static __virt_start: u8;
    static __virt_end: u8;
    static __code_start: u8;
    static __code_end: u8;
    static __rodata_start: u8;
    static __rodata_end: u8;
    static __data_start: u8;
    static __data_end: u8;
}

/// Initializes the kernel image metadata.
///
/// This function must be the first thing called, before calls to any other kernel/initialization
/// functions.
///
/// # Safety
///
/// Must be called only once at startup, and should not be called concurrently with other kimage
/// functions.
pub unsafe fn init(kernel_paddr: PhysAddr) {
    unsafe {
        KERNEL_PHYS = kernel_paddr.containing_frame();
    }
}

pub fn phys_base() -> PhysFrameNum {
    // Safety: no one else should be mutating `KERNEL_PHYS` at this point.
    unsafe { KERNEL_PHYS }
}

pub fn phys_end() -> PhysFrameNum {
    phys_base() + total_pages()
}

pub fn contains_phys(pfn: PhysFrameNum) -> bool {
    (phys_base()..phys_end()).contains(&pfn)
}

pub fn virt_base() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__virt_start }).containing_page()
}
pub fn virt_end() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__virt_end }).containing_page()
}
pub fn total_pages() -> usize {
    virt_end() - virt_base()
}

pub fn code_base() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__code_start }).containing_page()
}
pub fn code_end() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__code_end }).containing_page()
}
pub fn code_pages() -> usize {
    code_end() - code_base()
}

pub fn rodata_base() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__rodata_start }).containing_page()
}
pub fn rodata_end() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__rodata_end }).containing_page()
}
pub fn rodata_pages() -> usize {
    rodata_end() - rodata_base()
}

pub fn data_base() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__data_start }).containing_page()
}
pub fn data_end() -> VirtPageNum {
    VirtAddr::from_ptr(unsafe { &__data_end }).containing_page()
}
pub fn data_pages() -> usize {
    data_end() - data_base()
}

pub fn vpn_from_kernel_pfn(pfn: PhysFrameNum) -> VirtPageNum {
    let phys_base = phys_base();
    assert!(pfn >= phys_base);
    assert!(pfn < phys_base + total_pages());

    virt_base() + (pfn - phys_base)
}

pub fn pfn_from_kernel_vpn(vpn: VirtPageNum) -> PhysFrameNum {
    let virt_base = virt_base();
    assert!(vpn >= virt_base);
    assert!(vpn < virt_end());

    phys_base() + (vpn - virt_base)
}
