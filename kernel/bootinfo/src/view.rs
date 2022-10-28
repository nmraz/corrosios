use core::{iter, mem, slice};

use num_utils::align_up;

use crate::{Error, ItemHeader, ItemKind, ITEM_ALIGN};

/// A view into bootinfo data packed by a builder.
#[derive(Debug, Clone, Copy)]
pub struct View<'a> {
    buffer: &'a [u8],
}

impl<'a> View<'a> {
    /// Returs a new view suitable for reading bootinfo out of `buffer`.
    ///
    /// # Errors
    ///
    /// Returns an error if `buffer` is not suitably aligned.
    pub fn new(buffer: &'a [u8]) -> Result<Self, Error> {
        if buffer.as_ptr() as usize % ITEM_ALIGN != 0 {
            return Err(Error::BadAlign);
        }

        Ok(Self { buffer })
    }

    /// Returns the total size of the bootinfo covered by this view.
    pub fn size(&self) -> usize {
        self.buffer.len()
    }

    /// Returns an iterator over all the items in this bootinfo.
    ///
    /// # Panics
    ///
    /// The returned iterator will panic if it encounters malformed bootinfo ()
    pub fn items(&self) -> impl Iterator<Item = ItemView<'a>> + Clone {
        let buffer = self.buffer;
        let size = self.size();
        let mut off = 0;

        iter::from_fn(move || {
            if off >= size {
                return None;
            }

            let payload_off = off + mem::size_of::<ItemHeader>();

            // Safety: `ItemHeader` is a POD
            let header: &ItemHeader =
                unsafe { get_slice_ref(&buffer[off..payload_off]) }.expect("malformed bootinfo");

            debug_assert_eq!(payload_off % ITEM_ALIGN, 0);

            let payload_end_off = payload_off + header.payload_len as usize;
            let payload = &buffer[payload_off..payload_end_off];

            off = align_up(payload_end_off, ITEM_ALIGN);

            Some(ItemView {
                kind: header.kind,
                payload,
            })
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ItemView<'a> {
    kind: ItemKind,
    payload: &'a [u8],
}

impl<'a> ItemView<'a> {
    pub fn kind(&self) -> ItemKind {
        self.kind
    }

    pub fn payload(&self) -> &'a [u8] {
        self.payload
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// valid value of type `T`.
    pub unsafe fn get<T>(&self) -> Result<&'a T, Error> {
        unsafe { get_slice_ref(self.payload()) }
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// valid value of type `T`.
    pub unsafe fn read<T: Copy>(&self) -> Result<T, Error> {
        unsafe { self.get().map(|p| *p) }
    }

    /// # Safety
    ///
    /// `T` must have a stable, well-defined layout, and the contents of the payload must be a
    /// sequence of valid values of type `T`.
    pub unsafe fn get_slice<T>(&self) -> Result<&'a [T], Error> {
        let payload = self.payload();

        if payload.len() % mem::size_of::<T>() != 0 {
            return Err(Error::BadSize);
        }

        if payload.as_ptr() as usize % mem::align_of::<T>() != 0 {
            return Err(Error::BadAlign);
        }

        Ok(unsafe {
            slice::from_raw_parts(
                payload.as_ptr() as *const _,
                payload.len() / mem::size_of::<T>(),
            )
        })
    }
}

/// # Safety
///
/// `T` must have a stable, well-defined layout and the contents of the slice must be a valid value
/// of type `T`.
unsafe fn get_slice_ref<T>(slice: &[u8]) -> Result<&T, Error> {
    if mem::size_of::<T>() != slice.len() {
        return Err(Error::BadSize);
    }

    if slice.as_ptr() as usize % mem::align_of::<T>() != 0 {
        return Err(Error::BadAlign);
    }

    Ok(unsafe { &*(slice.as_ptr() as *const T) })
}
