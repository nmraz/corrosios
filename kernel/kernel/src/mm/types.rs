use crate::arch::mmu::{PAGE_SHIFT, PT_LEVEL_MASK, PT_LEVEL_SHIFT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PhysAddr(usize);

impl PhysAddr {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn containing_page(self) -> PhysPageNum {
        PhysPageNum::new(self.0 >> PAGE_SHIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn as_ptr<T>(self) -> *const T {
        assert!(self.0 % core::mem::align_of::<T>() == 0);
        self.0 as _
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as _
    }

    pub const fn containing_page(self) -> VirtPageNum {
        VirtPageNum::new(self.0 >> PAGE_SHIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PhysPageNum(usize);

impl PhysPageNum {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn addr(self) -> PhysAddr {
        PhysAddr::new(self.0 << PAGE_SHIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct VirtPageNum(usize);

impl VirtPageNum {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn addr(self) -> VirtAddr {
        VirtAddr::new(self.0 << PAGE_SHIFT)
    }

    pub const fn pt_index(self, level: usize) -> usize {
        (self.0 >> (PT_LEVEL_SHIFT * level + PAGE_SHIFT)) & PT_LEVEL_MASK
    }
}
