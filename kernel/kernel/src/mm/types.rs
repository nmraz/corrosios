use bitflags::bitflags;

use crate::arch::mmu::{PAGE_SHIFT, PT_LEVEL_MASK, PT_LEVEL_SHIFT};

bitflags! {
    pub struct PageTablePerms: u8 {
        const WRITE = 1 << 0;
        const EXECUTE = 1 << 1;
        const USER = 1 << 2;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VirtAddr(usize);

impl VirtAddr {
    pub const fn new(val: usize) -> Self {
        Self(val)
    }

    pub fn from_ptr<T>(p: *const T) -> Self {
        Self(p as usize)
    }

    pub fn from_mut_ptr<T>(p: *mut T) -> Self {
        Self(p as usize)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }

    pub const fn as_u64(self) -> u64 {
        self.0 as u64
    }

    pub const fn as_ptr<T>(self) -> *const T {
        self.0 as _
    }

    pub const fn as_mut_ptr<T>(self) -> *mut T {
        self.0 as _
    }

    pub const fn containing_page(self) -> VirtPageNum {
        VirtPageNum::new(self.0 >> PAGE_SHIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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
        (self.0 >> (PT_LEVEL_SHIFT * level)) & PT_LEVEL_MASK
    }
}

macro_rules! impl_add_sub_usize {
    ($t:ty) => {
        impl core::ops::Add<usize> for $t {
            type Output = $t;

            fn add(self, rhs: usize) -> $t {
                <$t>::new(self.as_usize() + rhs)
            }
        }

        impl core::ops::Add<$t> for usize {
            type Output = $t;

            fn add(self, rhs: $t) -> $t {
                <$t>::new(self + rhs.as_usize())
            }
        }

        impl core::ops::AddAssign<usize> for $t {
            fn add_assign(&mut self, rhs: usize) {
                self.0 += rhs;
            }
        }

        impl core::ops::Sub<usize> for $t {
            type Output = $t;

            fn sub(self, rhs: usize) -> $t {
                <$t>::new(self.as_usize() - rhs)
            }
        }

        impl core::ops::Sub for $t {
            type Output = usize;

            fn sub(self, rhs: $t) -> usize {
                self.as_usize() - rhs.as_usize()
            }
        }

        impl core::ops::SubAssign<usize> for $t {
            fn sub_assign(&mut self, rhs: usize) {
                self.0 -= rhs;
            }
        }
    };
}

impl_add_sub_usize!(PhysAddr);
impl_add_sub_usize!(VirtAddr);
impl_add_sub_usize!(PhysPageNum);
impl_add_sub_usize!(VirtPageNum);
