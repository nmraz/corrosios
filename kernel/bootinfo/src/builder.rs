use core::mem::{self, MaybeUninit};
use core::ptr;

use crate::{align_item_offset, ItemHeader, ItemKind, ITEM_ALIGN};

#[derive(Debug, Clone, Copy)]
pub enum BuildError {
    BadSize,
    BadAlign,
}

pub struct Builder<'a> {
    buf: &'a mut [u8],
    off: usize,
}

impl<'a> Builder<'a> {
    pub fn new(buf: &'a mut [u8]) -> Result<Self, BuildError> {
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

    pub fn append<T: ?Sized>(&mut self, kind: ItemKind, val: &T) -> Result<(), BuildError> {
        if mem::align_of_val(val) > ITEM_ALIGN {
            return Err(BuildError::BadAlign);
        }

        let size = mem::size_of_val(val);
        let total_size = size
            .checked_add(mem::size_of::<ItemHeader>())
            .ok_or(BuildError::BadSize)?;

        let off = align_item_offset(self.off);
        let next_off = off.checked_add(total_size).ok_or(BuildError::BadSize)?;

        if next_off > self.buf.len() {
            return Err(BuildError::BadSize);
        }

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
            ptr::copy_nonoverlapping(
                val as *const _ as *const u8,
                self.buf
                    .as_mut_ptr()
                    .add(off + mem::size_of::<ItemHeader>()),
                size,
            );
        }

        Ok(())
    }

    pub fn finish(self) -> &'a ItemHeader {
        // Safety: buffer size, alignment checked in `new`.
        let header = unsafe { &mut *(self.buf.as_mut_ptr() as *mut MaybeUninit<ItemHeader>) };
        header.write(ItemHeader {
            kind: ItemKind::CONTAINER,
            payload_len: self.off as u32,
        })
    }
}
