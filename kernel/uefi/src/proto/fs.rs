use core::marker::PhantomData;
use core::{mem, ptr};

use bitflags::bitflags;
use uninit::out_ref::Out;

use crate::{guid, Guid, Result, Status, Timestamp, U16CStr};

use super::{abi_call, unsafe_protocol, Protocol};

#[repr(C)]
struct FileAbi {
    revision: u64,
    open: unsafe extern "efiapi" fn(*mut Self, *mut *mut FileAbi, *const u16, u64, u64) -> Status,
    close: unsafe extern "efiapi" fn(*mut Self) -> Status,
    delete: unsafe extern "efiapi" fn(*mut Self) -> Status,
    read: unsafe extern "efiapi" fn(*mut Self, *mut usize, *mut u8) -> Status,
    write: unsafe extern "efiapi" fn(*mut Self, *mut usize, *const u8) -> Status,
    get_position: unsafe extern "efiapi" fn(*mut Self, *mut u64) -> Status,
    set_position: unsafe extern "efiapi" fn(*mut Self, u64) -> Status,
    get_info: unsafe extern "efiapi" fn(*mut Self, *const Guid, *mut usize, *mut u8) -> Status,
    set_info: unsafe extern "efiapi" fn(*mut Self, *const Guid, usize, *mut u8) -> Status,
    flush: unsafe extern "efiapi" fn(*mut Self) -> Status,
}

#[repr(C)]
pub struct SimpleFileSystemAbi {
    revision: u64,
    open_volume: unsafe extern "efiapi" fn(*mut Self, *mut *mut FileAbi) -> Status,
}

unsafe_protocol! {
    SimpleFileSystem(SimpleFileSystemAbi, "964e5b22-6459-11d2-8e39-00a0c969723b");
}

impl SimpleFileSystem {
    pub fn open_volume(&self) -> Result<File<'_>> {
        let mut abi = ptr::null_mut();
        unsafe {
            abi_call!(self, open_volume(&mut abi)).to_result()?;
            Ok(File::new(abi))
        }
    }
}

pub struct File<'a>(*mut FileAbi, PhantomData<&'a ()>);

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct OpenMode: u64 {
        const READ = 1;
        const WRITE = 2;
    }
}

const GUID_FILE_INFO: Guid = guid!("09576e92-6d3f-11d2-8e39-00a0c969723b");

impl File<'_> {
    unsafe fn new(abi: *mut FileAbi) -> Self {
        Self(abi, PhantomData)
    }

    fn abi(&self) -> *mut FileAbi {
        self.0
    }

    pub fn open(&self, name: &U16CStr, mode: OpenMode) -> Result<File<'_>> {
        let mut abi = ptr::null_mut();
        unsafe {
            abi_call!(self, open(&mut abi, name.as_ptr(), mode.bits(), 0)).to_result()?;
            Ok(File::new(abi))
        }
    }

    pub fn read<'a>(&mut self, mut buf: Out<'a, [u8]>) -> Result<&'a [u8]> {
        let mut size = buf.len();
        unsafe { abi_call!(self, read(&mut size, buf.as_mut_ptr())) }.to_result()?;

        // Safety: this portion of the buffer has been initialized by the call to `read` above.
        Ok(unsafe { buf.get_out(..size).unwrap().assume_all_init() })
    }

    pub fn read_exact<'a>(&mut self, buf: Out<'a, [u8]>) -> Result<&'a [u8]> {
        let len = buf.len();
        let res = self.read(buf)?;

        if res.len() != len {
            Err(Status::END_OF_FILE)
        } else {
            Ok(res)
        }
    }

    pub fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut size = buf.len();
        unsafe { abi_call!(self, write(&mut size, buf.as_ptr())) }.to_result()?;
        Ok(size)
    }

    pub fn position(&self) -> Result<u64> {
        let mut pos = 0;
        unsafe { abi_call!(self, get_position(&mut pos)) }.to_result()?;
        Ok(pos)
    }

    pub fn set_position(&mut self, pos: u64) -> Result<()> {
        unsafe { abi_call!(self, set_position(pos)) }.to_result()
    }

    pub fn info_size(&self) -> Result<usize> {
        let mut size = 0;
        let status =
            unsafe { abi_call!(self, get_info(&GUID_FILE_INFO, &mut size, ptr::null_mut())) };

        if status != Status::BUFFER_TOO_SMALL {
            status.to_result()?;
        }

        Ok(size)
    }

    pub fn info<'a>(&self, mut buf: Out<'a, [u8]>) -> Result<&'a FileInfo> {
        assert_eq!(buf.as_ptr() as usize % mem::align_of::<FileInfo>(), 0);

        let mut size = buf.len();
        unsafe { abi_call!(self, get_info(&GUID_FILE_INFO, &mut size, buf.as_mut_ptr())) }
            .to_result()?;

        Ok(unsafe { &*(buf.as_ptr() as *const _) })
    }
}

impl Drop for File<'_> {
    fn drop(&mut self) {
        let _ = unsafe { abi_call!(self, close()) };
    }
}

#[repr(C)]
pub struct FileInfo {
    info_size: u64,
    file_size: u64,
    physical_size: u64,
    create_time: Timestamp,
    last_access_time: Timestamp,
    modification_time: Timestamp,
    attr: u64,
}

impl FileInfo {
    pub fn size(&self) -> u64 {
        self.file_size
    }

    pub fn physical_size(&self) -> u64 {
        self.physical_size
    }

    pub fn attr(&self) -> u64 {
        self.attr
    }

    pub fn name(&self) -> &U16CStr {
        unsafe {
            let name_start = (self as *const Self).add(1).cast();
            U16CStr::from_ptr(name_start)
        }
    }

    pub fn create_time(&self) -> &Timestamp {
        &self.create_time
    }

    pub fn last_access_time(&self) -> &Timestamp {
        &self.last_access_time
    }

    pub fn modification_time(&self) -> &Timestamp {
        &self.modification_time
    }
}
