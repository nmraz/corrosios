#![no_std]

pub const MAGIC: [u8; 4] = *b"\x7fELF";
pub const CLASS_64: u8 = 2;
pub const DATA_LE: u8 = 1;
pub const IDENT_VERSION_CURRENT: u8 = 1;
pub const ABI_SYSV: u8 = 0;
pub const ABI_VERSION_CURRENT: u8 = 0;
pub const VERSION_CURRENT: u32 = 1;

pub const ELF_TYPE_EXEC: u16 = 2;
pub const ELF_TYPE_DYN: u16 = 3;

pub const SEGMENT_TYPE_NULL: u32 = 0;
pub const SEGMENT_TYPE_LOAD: u32 = 1;

pub const SEGMENT_FLAG_READ: u32 = 4;
pub const SEGMENT_FLAG_WRITE: u32 = 2;
pub const SEGMENT_FLAG_EXEC: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Header {
    pub magic: [u8; 4],
    pub class: u8,
    pub data: u8,
    pub ident_version: u8,
    pub abi: u8,
    pub abi_version: u8,
    pub pad: [u8; 7],
    pub ty: u16,
    pub machine: u16,
    pub version: u32,
    pub entry: u64,
    pub ph_off: u64,
    pub sh_off: u64,
    pub flags: u32,
    pub header_size: u16,
    pub ph_entry_size: u16,
    pub ph_entry_num: u16,
    pub sh_entry_size: u16,
    pub sh_entry_num: u16,
    pub sh_str_index: u16,
}

impl Header {
    pub fn is_valid(&self) -> bool {
        self.magic == MAGIC
            && self.class == CLASS_64
            && self.data == DATA_LE
            && self.ident_version == IDENT_VERSION_CURRENT
            && self.abi == ABI_SYSV
            && self.abi_version == ABI_VERSION_CURRENT
            && self.version == VERSION_CURRENT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProgramHeader {
    pub ty: u32,
    pub flags: u32,
    pub off: u64,
    pub virt_addr: u64,
    pub phys_addr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub align: u64,
}
