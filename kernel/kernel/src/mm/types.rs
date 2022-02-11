use crate::arch::mmu::PAGE_SHIFT;

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

    pub const fn containing_frame(self) -> PhysFrame {
        PhysFrame::new(self.0 >> PAGE_SHIFT)
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

    pub const fn containing_frame(self) -> VirtFrame {
        VirtFrame::new(self.0 >> PAGE_SHIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PhysFrame(usize);

impl PhysFrame {
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
pub struct VirtFrame(usize);

impl VirtFrame {
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
}
