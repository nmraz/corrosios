use core::mem::{self, MaybeUninit};
use core::{ptr, slice};

use num_utils::align_up;
use uninit::extension_traits::AsOut;
use uninit::out_ref::Out;

use crate::{Error, ItemHeader, ItemKind, ITEM_ALIGN};

pub struct Builder<'a> {
    buffer: Out<'a, [u8]>,
    off: usize,
}

impl<'a> Builder<'a> {
    pub fn new(buffer: Out<'a, [u8]>) -> Result<Self, Error> {
        if buffer.as_ptr() as usize % ITEM_ALIGN != 0 {
            return Err(Error::BadAlign);
        }

        let len = buffer.len();
        if len >= i32::MAX as usize {
            return Err(Error::BadSize);
        }

        Ok(Self { buffer, off: 0 })
    }

    /// # Safety
    ///
    /// The caller must initialize the entire buffer reserved.
    pub unsafe fn reserve<T>(
        &mut self,
        kind: ItemKind,
        count: usize,
    ) -> Result<&mut [MaybeUninit<T>], Error> {
        if mem::align_of::<T>() > ITEM_ALIGN {
            return Err(Error::BadAlign);
        }

        let size = mem::size_of::<T>()
            .checked_mul(count)
            .ok_or(Error::BadSize)?;

        let total_size = size
            .checked_add(mem::size_of::<ItemHeader>())
            .ok_or(Error::BadSize)?;

        let off = align_up(self.off, ITEM_ALIGN);
        let next_off = off.checked_add(total_size).ok_or(Error::BadSize)?;

        if next_off > self.buffer.len() {
            return Err(Error::BadSize);
        }

        self.off = next_off;

        // Safety: offset has been checked, pointer is suitably aligned thanks to `align_to_offset`.
        unsafe {
            ptr::write(
                self.buffer.as_mut_ptr().add(off) as *mut _,
                ItemHeader {
                    kind,
                    payload_len: size as u32,
                },
            );
        }

        // Safety: alignment and validity of offset checked above.
        unsafe {
            Ok(slice::from_raw_parts_mut(
                self.buffer
                    .as_mut_ptr()
                    .add(off + mem::size_of::<ItemHeader>())
                    .cast(),
                count,
            ))
        }
    }

    pub fn append<T>(&mut self, kind: ItemKind, val: T) -> Result<(), Error> {
        // Safety: the single reserved element is initialized below.
        let buf = unsafe { self.reserve(kind, 1)? };
        buf[0].write(val);
        Ok(())
    }

    pub fn append_slice<T: Copy>(&mut self, kind: ItemKind, val: &[T]) -> Result<(), Error> {
        // Safety: the buffer is initialized below.
        let buf = unsafe { self.reserve(kind, val.len())? };
        buf.as_out().copy_from_slice(val);
        Ok(())
    }

    pub fn finish(self) -> &'a [u8] {
        // Safety: this entire portion of the buffer should have been initialized by previous
        // calls to `append` and the like.
        unsafe {
            self.buffer
                .get_out(..self.off)
                .expect("offset exceeded buffer size")
                .assume_all_init()
        }
    }
}
