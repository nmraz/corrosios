use bitflags::bitflags;

bitflags! {
    #[repr(transparent)]
    pub struct DescriptorTableFlags: u64 {
        const WRITE = 1 << 41;
        const EXEC = 1 << 43;
        const NON_SYSTEM = 1 << 44;
        const RING3 = 3 << 45;
        const PRESENT = 1 << 47;
        const LONG_MODE = 1 << 53;
    }
}
