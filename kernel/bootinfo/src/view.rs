use core::marker::PhantomData;
use core::{iter, mem, slice};

use num_utils::align_up;

use crate::{ItemHeader, ItemKind, ITEM_ALIGN};

#[derive(Debug, Clone, Copy)]
pub struct BadMagic;

#[derive(Debug, Clone, Copy)]
pub struct InvalidPayload;

#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    // Note: we use a raw pointer here to avoid provenance issues
    header: *const ItemHeader,
    _marker: PhantomData<&'a ()>,
}

impl<'a> View<'a> {
    /// # Safety
    ///
    /// `header` must be the header of a valid boot info structure in memory. The boot info should
    /// not be mutated for the remainder of `'a`.
    pub unsafe fn new(header: *const ItemHeader) -> Result<Self, BadMagic> {
        let header_ref = unsafe { &*header };
        if header_ref.kind != ItemKind::CONTAINER {
            Err(BadMagic)
        } else {
            Ok(Self {
                header,
                _marker: PhantomData,
            })
        }
    }

    pub fn content_size(&self) -> usize {
        // Safety: guaranteed by the contract of `new`
        unsafe { *self.header }.payload_len as usize
    }

    pub fn total_size(&self) -> usize {
        self.content_size() + mem::size_of::<ItemHeader>()
    }

    pub fn items(&self) -> impl Iterator<Item = ItemView<'a>> + Clone {
        // Safety: we can always move to the byte just past the end of the allocation (though there
        // will generally be additional payload data after the header).
        let base = unsafe { self.header.add(1) as *const u8 };

        let len = self.content_size();
        let mut off = 0;

        iter::from_fn(move || {
            if off >= len {
                return None;
            }

            // Safety: per the safety contract of `new`, this offset should still point into the
            // allocation and point to a valid `ItemHeader`.
            let header = unsafe { &*(base.add(off) as *const ItemHeader) };
            off = align_up(
                off + mem::size_of::<ItemHeader>() + header.payload_len as usize,
                ITEM_ALIGN,
            );

            Some(ItemView {
                header,
                _marker: PhantomData,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ItemView<'a> {
    header: *const ItemHeader,
    _marker: PhantomData<&'a ()>,
}

impl<'a> ItemView<'a> {
    pub fn kind(&self) -> ItemKind {
        unsafe { *self.header }.kind
    }

    pub fn payload(&self) -> &'a [u8] {
        // Safety: this item view could only have been created by a `View`, and the contract of
        // `View::new` requires that the boot info be valid (in particular, every header is followed
        // by `payload_len` bytes of payload).
        unsafe {
            slice::from_raw_parts(
                self.header.add(1) as *const _,
                (*self.header).payload_len as usize,
            )
        }
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// valid value of type `T`.
    pub unsafe fn get<T>(&self) -> Result<&'a T, InvalidPayload> {
        let payload = self.payload();
        if payload.len() != mem::size_of::<T>()
            || payload.as_ptr() as usize % mem::align_of::<T>() != 0
        {
            return Err(InvalidPayload);
        }

        Ok(unsafe { &*(payload.as_ptr() as *const T) })
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// valid value of type `T`.
    pub unsafe fn read<T: Copy>(&self) -> Result<T, InvalidPayload> {
        unsafe { self.get().map(|p| *p) }
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// sequence of valid values of type `T`.
    pub unsafe fn get_slice<T>(&self) -> Result<&'a [T], InvalidPayload> {
        let payload = self.payload();
        if payload.len() % mem::size_of::<T>() != 0
            || payload.as_ptr() as usize % mem::align_of::<T>() != 0
        {
            return Err(InvalidPayload);
        }

        Ok(unsafe {
            slice::from_raw_parts(
                payload.as_ptr() as *const _,
                payload.len() / mem::size_of::<T>(),
            )
        })
    }
}
