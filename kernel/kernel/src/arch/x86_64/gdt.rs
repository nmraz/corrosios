use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    pub struct DescriptorFlags: u64 {
        const WRITE = 1 << 41;
        const EXEC = 1 << 43;
        const NON_SYSTEM = 1 << 44;
        const RING3 = 3 << 45;
        const PRESENT = 1 << 47;
        const LONG_MODE = 1 << 53;
    }
}

pub const GDT_SIZE: usize = 2;

#[no_mangle]
pub static GDT: [DescriptorFlags; GDT_SIZE] = [
    // Null descriptor
    DescriptorFlags::empty(),
    // Kernel code segment
    DescriptorFlags::from_bits_truncate(
        DescriptorFlags::NON_SYSTEM.bits()
            | DescriptorFlags::PRESENT.bits()
            | DescriptorFlags::EXEC.bits()
            | DescriptorFlags::LONG_MODE.bits(),
    ),
];
