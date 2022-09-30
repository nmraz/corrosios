use core::mem::{self, MaybeUninit};
use core::ptr::addr_of_mut;

use bitflags::bitflags;
use paste::paste;
use spin_once::Once;

use crate::mm::types::VirtAddr;

use super::interrupt_vectors::{TOTAL_VECTORS, VECTOR_DOUBLE_FAULT, VECTOR_NMI};

pub const IOPB_BITS: usize = 0x10000;
pub const IOPB_BYTES: usize = bitmap::bytes_required(IOPB_BITS);

#[repr(C, packed)]
struct TssFixed {
    _reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    _reserved1: u32,
    _reserved2: u32,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    _reserved3: u32,
    _reserved4: u32,
    _reserved5: u16,
    iopb_base: u16,
}

// Note: keep these IST numbers in sync with the TSS construction below
const IST_NMI: u8 = 1;
const IST_DOUBLE_FAULT: u8 = 2;

/// 64-bit Task State Segment structure, as specified in ISDM 3A, section 7.7
#[repr(C, packed)]
pub struct Tss {
    fixed: TssFixed,

    // We always place the IOPB immediately after the fixed portion, with an extra trailing `0xff`
    // byte as specified by ISDM Vol 1, section 19.5.2
    iopb: [u8; IOPB_BYTES + 1],
}

impl Tss {
    /// # Safety
    ///
    /// `tss` must be suitably aligned and dereferenceable
    pub unsafe fn init(tss: *mut Tss, nmi_stack: VirtAddr, double_fault_stack: VirtAddr) {
        unsafe {
            let fixed = addr_of_mut!((*tss).fixed);
            fixed.write(TssFixed {
                _reserved0: 0,
                rsp0: 0,
                rsp1: 0,
                rsp2: 0,
                _reserved1: 0,
                _reserved2: 0,
                ist1: nmi_stack.as_u64(),
                ist2: double_fault_stack.as_u64(),
                ist3: 0,
                ist4: 0,
                ist5: 0,
                ist6: 0,
                ist7: 0,
                _reserved3: 0,
                _reserved4: 0,
                _reserved5: 0,
                iopb_base: mem::size_of::<TssFixed>() as u16,
            });

            let iopb = addr_of_mut!((*tss).iopb);
            iopb.write_bytes(0xff, 1);
        }
    }

    pub fn set_rsp0(&mut self, rsp0: VirtAddr) {
        self.fixed.rsp0 = rsp0.as_u64();
    }

    pub fn set_iopb(&mut self, iopb: &[u8; IOPB_BYTES]) {
        self.iopb[..IOPB_BYTES].copy_from_slice(iopb);
    }
}

const GDT_ENTRIES: usize = 4;

// Note: keep these selectors in sync with the GDT entries below
pub const KERNEL_CODE_SELECTOR: u16 = 8;
pub const TSS_SELECTOR: u16 = 16;

#[derive(Debug, Clone, Copy)]
#[repr(transparent)]
pub struct Gdt([u64; GDT_ENTRIES]);

impl Gdt {
    pub fn new(tss: VirtAddr) -> Self {
        let (tss_lo, tss_hi) = make_gdt_tss_descriptor(tss.as_u64());

        Self([
            // Null segment
            0,
            // Kernel code segment
            make_gdt_non_system_descriptor(GdtFlags::TYPE_CODE | GdtFlags::LONG_MODE),
            // TSS segment
            tss_lo,
            tss_hi,
        ])
    }
}

fn make_gdt_non_system_descriptor(flags: GdtFlags) -> u64 {
    (flags | GdtFlags::NON_SYSTEM | GdtFlags::PRESENT).bits()
}

fn make_gdt_tss_descriptor(base: u64) -> (u64, u64) {
    // See ISDM 3A, section 7.2.3

    let flags = GdtFlags::PRESENT | GdtFlags::TYPE_TSS;
    let limit = (mem::size_of::<Tss>() - 1) as u64;

    let base_32_63 = base >> 32;
    let base_0_23 = base & 0xff_ffff;
    let base_24_31 = (base >> 24) & 0xff;

    let limit_0_15 = limit & 0xffff;
    let limit_16_19 = (limit >> 16) & 0xf;

    (
        flags.bits() | limit_0_15 | (base_0_23 << 16) | limit_16_19 | base_24_31,
        base_32_63,
    )
}

bitflags! {
    struct GdtFlags: u64 {
        const WRITE = 1 << 41;
        const NON_SYSTEM = 1 << 44;
        const RING3 = 3 << 45;
        const PRESENT = 1 << 47;
        const LONG_MODE = 1 << 53;

        const TYPE_CODE = 1 << 43;
        const TYPE_TSS = 0b1001 << 40;
    }
}

type IdtEntry = [u64; 2];
type Idt = [IdtEntry; TOTAL_VECTORS];
static IDT: Once<Idt> = Once::new();

pub fn init_idt() {
    use super::interrupt::entry_points::*;

    macro_rules! idt_entry {
        ($vector:literal, $slots:ident) => {
            paste! {
                {
                    let entry_point = [<interrupt_vector_ $vector>] as unsafe extern "C" fn() as u64;
                    $slots[$vector].write(make_idt_entry(entry_point, KERNEL_CODE_SELECTOR, get_ist($vector)));
                }
            }
        };
    }

    unsafe {
        IDT.init_with(|slot| {
            let slots = slot
                .as_mut_ptr()
                .cast::<[MaybeUninit<IdtEntry>; TOTAL_VECTORS]>()
                .as_mut()
                .unwrap();

            for_each_interrupt!(idt_entry, slots);
        });
    }
}

pub fn get_idt() -> VirtAddr {
    VirtAddr::from_ptr(IDT.get().expect("IDT not initialized"))
}

pub fn get_idt_size() -> usize {
    mem::size_of::<Idt>()
}

fn get_ist(vector: u64) -> u8 {
    match vector {
        VECTOR_NMI => IST_NMI,
        VECTOR_DOUBLE_FAULT => IST_DOUBLE_FAULT,
        _ => 0,
    }
}

fn make_idt_entry(entry_point: u64, cs_selector: u16, ist: u8) -> IdtEntry {
    let selector = cs_selector as u64;
    let ist = (ist & 0b111) as u64;

    let offset_0_15 = entry_point & 0xffff;
    let offset_16_31 = (entry_point >> 16) & 0xffff;
    let offset_32_63 = entry_point >> 32;

    let flags = IdtFlags::PRESENT | IdtFlags::TYPE_INTERRUPT_64;

    [
        flags.bits() | offset_0_15 | (selector << 16) | (ist << 32) | (offset_16_31 << 48),
        offset_32_63,
    ]
}

bitflags! {
    struct IdtFlags: u64 {
        const PRESENT = 1 << 47;
        const TYPE_INTERRUPT_64 = 0b1110 << 40;
    }
}
