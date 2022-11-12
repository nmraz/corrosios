use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::err::{Error, Result};
use crate::mm::pmm;
use crate::mm::types::{CacheMode, PhysFrameNum};
use crate::sync::SpinLock;

/// Access type hint used when requesting pages from a [`VmObject`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitType {
    /// The requested pages will not be written to, only read or executed.
    Read,
    /// The requested pages may be written to and should be prepared to handle those cases.
    Write,
}

/// A virtual memory object that can be mapped into an address space.
///
/// # Safety
///
/// * The implementation of [`provide_page`](VmObject::provide_page) must return a frame that can be
///   safely used by clients mapping the object
/// * The implementation of [`cache_mode`](VmObject::cache_mode) must return a cache mode that can
///   safely be applied to the provided pages, respecting any platform limitations.
pub unsafe trait VmObject: Send + Sync {
    /// Retrieves the size of this VM object, in pages.
    fn page_count(&self) -> usize;

    /// Requests the page at offset `offset` within the object, assuming it will be accessed in
    /// accordance with `commit_type`.
    ///
    /// For now, this function should not block as it will be called with a spinlock held.
    fn provide_page(&self, offset: usize, commit_type: CommitType) -> Result<PhysFrameNum>;

    /// Returns the cache mode that should be used when mapping this object.
    ///
    /// By default, returns [`CacheMode::WriteBack`], which is suitable for "ordinary" (non-IO)
    /// memory.
    fn cache_mode(&self) -> CacheMode {
        CacheMode::WriteBack
    }
}

/// A VM object that allocates all of its backing page frames upon construction.
///
/// Prefer using this to [`LazyVmObject`] if the object is going to be committed in its entirety
/// immediately after being mapped (as is the case for kernel mappings), as it will use less memory
/// for redundant metadata.
pub struct EagerVmObject {
    frames: Vec<FrameBox>,
}

impl EagerVmObject {
    pub fn new(page_count: usize) -> Result<Arc<Self>> {
        let mut frames = Vec::new();
        frames.try_reserve_exact(page_count)?;

        for _ in 0..page_count {
            // Note: the `push` calls will never allocate as we have reserved enough space above.
            frames.push(FrameBox::new()?);
        }

        Ok(Arc::try_new(Self { frames })?)
    }
}

unsafe impl VmObject for EagerVmObject {
    fn page_count(&self) -> usize {
        self.frames.len()
    }

    fn provide_page(&self, offset: usize, _commit_type: CommitType) -> Result<PhysFrameNum> {
        Ok(self.frames[offset].0)
    }
}

/// A VM object that lazily allocates its backing page frames as they are requested.
///
/// If the entire object is going to be committed immediately when it is mapped (as is the case for
/// all kernel mappings), prefer [`EagerVmObject`], as it will behave identically but use less
/// memory for bookkeeping.
pub struct LazyVmObject {
    page_count: usize,
    // TODO: maybe not a spinlock?
    frames: SpinLock<Vec<Option<FrameBox>>>,
}

impl LazyVmObject {
    pub fn new(page_count: usize) -> Result<Arc<Self>> {
        let mut frames = Vec::new();
        frames.try_reserve_exact(page_count)?;

        for _ in 0..page_count {
            // Note: the `push` calls will never allocate as we have reserved enough space above.
            frames.push(None);
        }

        Ok(Arc::try_new(Self {
            page_count,
            frames: SpinLock::new(frames),
        })?)
    }
}

unsafe impl VmObject for LazyVmObject {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn provide_page(&self, offset: usize, _commit_type: CommitType) -> Result<PhysFrameNum> {
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

/// A VM object backed by a contiguous range of physical memory.
pub struct PhysVmObject {
    base: PhysFrameNum,
    page_count: usize,
    cache_mode: CacheMode,
}

impl PhysVmObject {
    /// # Safety
    ///
    /// The caller must guarantee that the specified range of physical memory is safe to access with
    /// the specified cache mode.
    pub unsafe fn new(
        base: PhysFrameNum,
        page_count: usize,
        cache_mode: CacheMode,
    ) -> Result<Arc<Self>> {
        Ok(Arc::try_new(Self {
            base,
            page_count,
            cache_mode,
        })?)
    }
}

unsafe impl VmObject for PhysVmObject {
    fn page_count(&self) -> usize {
        self.page_count
    }

    fn provide_page(&self, offset: usize, _commit_type: CommitType) -> Result<PhysFrameNum> {
        assert!(offset < self.page_count);
        Ok(self.base + offset)
    }

    fn cache_mode(&self) -> CacheMode {
        self.cache_mode
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
