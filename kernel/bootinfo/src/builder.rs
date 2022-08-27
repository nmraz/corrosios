use core::mem::{self, MaybeUninit};
use core::{ptr, slice};

use num_utils::align_up;
use uninit::extension_traits::AsOut;
use uninit::out_ref::Out;

use crate::{ItemHeader, ItemKind, ITEM_ALIGN};

#[derive(Debug, Clone, Copy)]
pub enum BuildError {
    BadSize,
    BadAlign,
}

pub struct Builder<'a> {
    buf: Out<'a, [u8]>,
    off: usize,
}

impl<'a> Builder<'a> {
    pub fn new(buf: Out<'a, [u8]>) -> Result<Self, BuildError> {
        if buf.as_ptr() as usize % ITEM_ALIGN != 0 {
            return Err(BuildError::BadAlign);
        }

        let len = buf.len();
        if len < mem::size_of::<ItemHeader>() || len >= i32::MAX as usize {
            return Err(BuildError::BadSize);
        }

        Ok(Self {
            buf,
            off: mem::size_of::<ItemHeader>(),
        })
    }

    /// # Safety
    ///
    /// The caller must initialize the entire buffer reserved.
    pub unsafe fn reserve<T>(
        &mut self,
        kind: ItemKind,
        count: usize,
    ) -> Result<&mut [MaybeUninit<T>], BuildError> {
        if mem::align_of::<T>() > ITEM_ALIGN {
            return Err(BuildError::BadAlign);
        }

        let size = mem::size_of::<T>()
            .checked_mul(count)
            .ok_or(BuildError::BadSize)?;

        let total_size = size
            .checked_add(mem::size_of::<ItemHeader>())
            .ok_or(BuildError::BadSize)?;

        let off = align_up(self.off, ITEM_ALIGN);
        let next_off = off.checked_add(total_size).ok_or(BuildError::BadSize)?;

        if next_off > self.buf.len() {
            return Err(BuildError::BadSize);
        }

        self.off = next_off;

        // Safety: offset has been checked, pointer is suitably aligned thanks to `align_to_offset`.
        unsafe {
            ptr::write(
                self.buf.as_mut_ptr().add(off) as *mut _,
                ItemHeader {
                    kind,
                    payload_len: size as u32,
                },
            );
        }

        // Safety: alignment and validity of offset checked above.
        unsafe {
            Ok(slice::from_raw_parts_mut(
                self.buf
                    .as_mut_ptr()
                    .add(off + mem::size_of::<ItemHeader>())
                    .cast(),
                count,
            ))
        }
    }

    pub fn append<T>(&mut self, kind: ItemKind, val: T) -> Result<(), BuildError> {
        // Safety: the single reserved element is initialized below.
        let buf = unsafe { self.reserve(kind, 1)? };
        buf[0].write(val);
        Ok(())
    }

    pub fn append_slice<T: Copy>(&mut self, kind: ItemKind, val: &[T]) -> Result<(), BuildError> {
        // Safety: the buffer is initialized below.
        let buf = unsafe { self.reserve(kind, val.len())? };
        buf.as_out().copy_from_slice(val);
        Ok(())
    }

    pub fn finish(mut self) -> &'a ItemHeader {
        // Safety: buffer size, alignment checked in `new`.
        let header = unsafe { &mut *(self.buf.as_mut_ptr() as *mut MaybeUninit<ItemHeader>) };
        header.write(ItemHeader {
            kind: ItemKind::CONTAINER,
            payload_len: self.off as u32,
        })
    }
}
