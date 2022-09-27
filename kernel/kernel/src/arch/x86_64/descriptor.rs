use bitflags::bitflags;

pub const IOPB_BITS: usize = 0x10000;
pub const IOPB_BYTES: usize = bitmap::bytes_required(IOPB_BITS);

/// 64-bit Task State Segment structure, as specified in ISDM 3A, section 7.7
#[derive(Debug, Clone, Copy)]
#[repr(C, packed)]
pub struct Tss {
    // Fixed portion
    pub _reserved0: u32,
    pub rsp0: u64,
    pub rsp1: u64,
    pub rsp2: u64,
    pub _reserved1: u32,
    pub _reserved2: u32,
    pub ist1: u64,
    pub ist2: u64,
    pub ist3: u64,
    pub ist4: u64,
    pub ist5: u64,
    pub ist6: u64,
    pub ist7: u64,
    pub _reserved3: u32,
    pub _reserved4: u32,
    pub _reserved5: u16,
    pub iopb_base: u16,

    // We always place the IOPB immediately after the fixed portion, with an extra trailing `0xff`
    // byte as specified by ISDM Vol 1, section 19.5.2
    pub iopb: [u8; IOPB_BYTES + 1],
}

bitflags! {
    #[repr(transparent)]
    pub struct GdtFlags: u64 {
        const WRITE = 1 << 41;
        const EXEC = 1 << 43;
        const NON_SYSTEM = 1 << 44;
        const RING3 = 3 << 45;
        const PRESENT = 1 << 47;
        const LONG_MODE = 1 << 53;
    }
}
