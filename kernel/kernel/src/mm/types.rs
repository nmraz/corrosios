use core::{fmt, ops};

use bitflags::bitflags;
use num_utils::{align_down, align_up};

use crate::arch::mmu::{PAGE_SHIFT, PAGE_SIZE, PT_LEVEL_MASK, PT_LEVEL_SHIFT};

use super::utils::write_flag;

bitflags! {
    /// Protection that can be applied to a VM object.
    #[derive(Clone, Copy)]
    pub struct Protection: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
    }
}

/// Caching modes that can be applied to a range of memory.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    /// "Full" cache operation: reads are cached, writes update the cache and can be written back
    /// later.
    ///
    /// This is the mode that should be used for all system (non-IO) memory.
    Cached,

    /// Reads are cached, but writes always write back to system memory in addition to possibly
    /// updating the cache.
    WriteThrough,

    /// Reads are not cached and always read from memory, writes are batched and written in larger
    /// chunks.
    WriteCombining,

    /// Neither reads nor writes use the cache; all operations access memory directly.
    Uncached,
}

impl fmt::Debug for CacheMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cached => write!(f, "C"),
            Self::WriteThrough => write!(f, "WT"),
            Self::WriteCombining => write!(f, "WC"),
            Self::Uncached => write!(f, "UC"),
        }
    }
}

/// The types of memory accesses that can cause a page fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Read,
    Write,
    Execute,
}

/// The processor mode in which a page fault can occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessMode {
    User,
    Kernel,
}

bitflags! {
    /// Low-level page table permissions.
    #[derive(Clone, Copy)]
    pub struct PageTablePerms: u8 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXECUTE = 1 << 2;
        const USER = 1 << 3;
        const GLOBAL = 1 << 4;
    }
}

impl fmt::Debug for PageTablePerms {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_flag(f, self.contains(Self::GLOBAL), 'g')?;
        write_flag(f, self.contains(Self::READ), 'r')?;
        write_flag(f, self.contains(Self::WRITE), 'w')?;
        write_flag(f, self.contains(Self::EXECUTE), 'x')?;
        write_flag(f, self.contains(Self::USER), 'u')?;

        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const fn frame_offset(self) -> usize {
        self.0 % PAGE_SIZE
    }

    pub const fn containing_frame(self) -> PhysFrameNum {
        PhysFrameNum::new(self.0 >> PAGE_SHIFT)
    }

    pub const fn containing_tail_frame(self) -> PhysFrameNum {
        PhysFrameNum::new((self.0 + PAGE_SIZE - 1) >> PAGE_SHIFT)
    }

    pub fn to_virt(self, f: impl FnOnce(PhysFrameNum) -> VirtPageNum) -> VirtAddr {
        f(self.containing_frame()).addr() + self.frame_offset()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

    pub const fn page_offset(self) -> usize {
        self.0 % PAGE_SIZE
    }

    pub const fn containing_page(self) -> VirtPageNum {
        VirtPageNum::new(self.0 >> PAGE_SHIFT)
    }

    pub const fn containing_tail_page(self) -> VirtPageNum {
        VirtPageNum::new((self.0 + PAGE_SIZE - 1) >> PAGE_SHIFT)
    }

    pub fn to_phys(self, f: impl FnOnce(VirtPageNum) -> PhysFrameNum) -> PhysAddr {
        f(self.containing_page()).addr() + self.page_offset()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PhysFrameNum(usize);

impl PhysFrameNum {
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

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
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

macro_rules! impl_arith_helpers {
    ($t:ty) => {
        impl $t {
            pub const fn align_down(self, align: usize) -> Self {
                Self(align_down(self.0, align))
            }

            pub const fn align_up(self, align: usize) -> Self {
                Self(align_up(self.0, align))
            }

            pub fn checked_add(self, rhs: usize) -> Option<Self> {
                self.0.checked_add(rhs).map(Self)
            }
        }

        impl fmt::Display for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                core::write!(f, "{:#x}", self.as_usize())
            }
        }

        impl fmt::Debug for $t {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(self, f)
            }
        }

        impl ops::Add<usize> for $t {
            type Output = $t;

            fn add(self, rhs: usize) -> $t {
                <$t>::new(self.as_usize() + rhs)
            }
        }

        impl ops::Add<$t> for usize {
            type Output = $t;

            fn add(self, rhs: $t) -> $t {
                <$t>::new(self + rhs.as_usize())
            }
        }

        impl ops::AddAssign<usize> for $t {
            fn add_assign(&mut self, rhs: usize) {
                self.0 += rhs;
            }
        }

        impl ops::Sub<usize> for $t {
            type Output = $t;

            fn sub(self, rhs: usize) -> $t {
                <$t>::new(self.as_usize() - rhs)
            }
        }

        impl ops::Sub for $t {
            type Output = usize;

            fn sub(self, rhs: $t) -> usize {
                self.as_usize() - rhs.as_usize()
            }
        }

        impl ops::SubAssign<usize> for $t {
            fn sub_assign(&mut self, rhs: usize) {
                self.0 -= rhs;
            }
        }
    };
}

impl_arith_helpers!(PhysAddr);
impl_arith_helpers!(VirtAddr);
impl_arith_helpers!(PhysFrameNum);
impl_arith_helpers!(VirtPageNum);
