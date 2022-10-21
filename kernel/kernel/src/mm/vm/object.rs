use alloc::vec::Vec;

use crate::err::{Error, Result};
use crate::mm::pmm;
use crate::mm::types::PhysFrameNum;
use crate::sync::SpinLock;

use super::AccessType;

/// A virtual memory object that can be mapped into an address space.
///
/// # Safety
///
/// The implementation of [`provide_page`](VmObject::provide_page) must return a frame that can be
/// safely used by clients mapping the object.
pub unsafe trait VmObject: Send + Sync {
    /// Retrieves the size of this VM object, in pages.
    fn page_count(&self) -> usize;

    /// Requests the page at offset `offset` within the object, assuming it will be accessed in
    /// accordance with `access_type`.
    ///
    /// For now, this function should not block as it will be called with a spinlock held.
    fn provide_page(&self, offset: usize, access_type: AccessType) -> Result<PhysFrameNum>;
}

pub struct EagerVmObject {
    frames: Vec<FrameBox>,
}

impl EagerVmObject {
    pub fn new(page_count: usize) -> Result<Self> {
        let mut frames = Vec::new();
        frames.try_reserve(page_count)?;

        for _ in 0..page_count {
            frames.push(FrameBox::new()?);
        }

        Ok(Self { frames })
    }
}

unsafe impl VmObject for EagerVmObject {
    fn page_count(&self) -> usize {
        self.frames.len()
    }

    fn provide_page(&self, offset: usize, _access_type: AccessType) -> Result<PhysFrameNum> {
        Ok(self.frames[offset].0)
    }
}

pub struct LazyVmObject {
    page_count: usize,
    // TODO: maybe not a spinlock?
    frames: SpinLock<Vec<Option<FrameBox>>>,
}

impl LazyVmObject {
    pub fn new(page_count: usize) -> Result<Self> {
        let mut frames = Vec::new();
        frames.try_reserve(page_count)?;

        Ok(Self {
            page_count,
            frames: SpinLock::new(frames),
        })
    }
}

unsafe impl VmObject for LazyVmObject {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn provide_page(&self, offset: usize, _access_type: AccessType) -> Result<PhysFrameNum> {
        self.frames.with(|frames, _| {
            let frame = match &frames[offset] {
                Some(frame) => frame.0,
                None => {
                    let frame = FrameBox::new()?;
                    let pfn = frame.0;
                    frames[offset] = Some(frame);
                    pfn
                }
            };

            Ok(frame)
        })
    }
}

pub struct PhysVmObject {
    base: PhysFrameNum,
    page_count: usize,
}

impl PhysVmObject {
    /// # Safety
    ///
    /// The caller must guarantee that the specified range of physical memory is safe to access.
    pub unsafe fn new(base: PhysFrameNum, page_count: usize) -> Self {
        Self { base, page_count }
    }
}

unsafe impl VmObject for PhysVmObject {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn provide_page(&self, offset: usize, _access_type: AccessType) -> Result<PhysFrameNum> {
        assert!(offset < self.page_count);
        Ok(self.base + offset)
    }
}

struct FrameBox(PhysFrameNum);

impl FrameBox {
    pub fn new() -> Result<Self> {
        pmm::allocate(0).map(Self).ok_or(Error::OUT_OF_MEMORY)
    }
}

impl Drop for FrameBox {
    fn drop(&mut self) {
        // Safety: this frame was obtained by a call to `pmm::allocate` with order 0.
        unsafe {
            pmm::deallocate(self.0, 0);
        }
    }
}
