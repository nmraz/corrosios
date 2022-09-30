use super::interrupt_vectors::{
    VECTOR_ALIGNMENT_CHECK, VECTOR_BOUND, VECTOR_BREAKPOINT, VECTOR_DEBUG, VECTOR_DEVICE_NOT_AVAIL,
    VECTOR_DIVIDE_ERROR, VECTOR_DOUBLE_FAULT, VECTOR_FPU_ERROR, VECTOR_GP_FAULT,
    VECTOR_INVALID_OPCODE, VECTOR_INVALID_TSS, VECTOR_MACHINE_CHECK, VECTOR_NMI, VECTOR_OVERFLOW,
    VECTOR_PAGE_FAULT, VECTOR_SEGMENT_NP, VECTOR_SIMD_ERROR, VECTOR_STACK_FAULT,
};

#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct InterruptFrame {
    // Saved state
    rax: u64,
    rbx: u64,
    rcx: u64,
    rdx: u64,
    rsi: u64,
    rdi: u64,
    rbp: u64,
    r8: u64,
    r9: u64,
    r10: u64,
    r11: u64,
    r12: u64,
    r13: u64,
    r14: u64,
    r15: u64,

    // Pushed by stub
    vector: u64,

    // Pushed by CPU or stub
    error_code: u64,

    // Fixed portion pushed by CPU upon entry
    rip: u64,
    cs: u64,
    rflags: u64,
    rsp: u64,
    ss: u64,
}

unsafe fn handle_exception(frame: &InterruptFrame) {
    panic!("fatal exception: {}", exception_vector_to_str(frame.vector));
}

fn exception_vector_to_str(vector: u64) -> &'static str {
    match vector {
        VECTOR_DIVIDE_ERROR => "division by 0",
        VECTOR_DEBUG => "debug exception",
        VECTOR_BREAKPOINT => "breakpoint",
        VECTOR_OVERFLOW => "integer overflow",
        VECTOR_BOUND => "bounds check failure",
        VECTOR_INVALID_OPCODE => "invalid opcode",
        VECTOR_DEVICE_NOT_AVAIL => "",
        VECTOR_DOUBLE_FAULT => "double fault",
        VECTOR_INVALID_TSS => "invalid TSS",
        VECTOR_SEGMENT_NP => "segment not present",
        VECTOR_STACK_FAULT => "stack fault",
        VECTOR_GP_FAULT => "general protection fault",
        VECTOR_PAGE_FAULT => "page fault",
        VECTOR_FPU_ERROR => "FPU floating-point error",
        VECTOR_ALIGNMENT_CHECK => "alignment check failure",
        VECTOR_MACHINE_CHECK => "machine check exception",
        VECTOR_SIMD_ERROR => "SIMD floating-point error",
        _ => "unknown exception",
    }
}

unsafe fn handle_nmi(_frame: &InterruptFrame) {}

unsafe fn handle_irq(frame: &InterruptFrame) {
    println!("got IRQ {}", frame.vector);
}

#[no_mangle]
unsafe extern "C" fn handle_interrupt(frame: &InterruptFrame) {
    unsafe {
        if frame.vector == VECTOR_NMI {
            handle_nmi(frame);
        } else if frame.vector < 32 {
            handle_exception(frame);
        } else {
            handle_irq(frame);
        }
    }
}

pub mod entry_points {
    use core::arch::global_asm;
    use paste::paste;

    use crate::arch::x86_64::interrupt_vectors::{
        VECTOR_ALIGNMENT_CHECK, VECTOR_DOUBLE_FAULT, VECTOR_GP_FAULT, VECTOR_INVALID_TSS,
        VECTOR_PAGE_FAULT, VECTOR_SEGMENT_NP, VECTOR_STACK_FAULT,
    };

    macro_rules! interrupt_stub {
        ($vector:literal) => {
            paste! {
                extern "C" {
                    pub fn [<interrupt_vector_ $vector>]();
                }
                global_asm!(
                    "
                .global interrupt_vector_{vector}
                .type interrupt_vector_{vector}, @function
                interrupt_vector_{vector}:
                    .if !{has_error_code}
                        // Error code placeholder
                        push 0
                    .endif
                    push {vector}
                    jmp interrupt_entry_common
                .size interrupt_vector_{vector}, interrupt_vector_{vector} - .
                ",
                    vector = const $vector,
                    has_error_code = const has_error_code($vector) as u32
                );
            }
        };
    }

    for_each_interrupt!(interrupt_stub);
    global_asm!(include_str!("interrupt.s"));

    const fn has_error_code(vector: u64) -> bool {
        matches!(
            vector,
            VECTOR_DOUBLE_FAULT
                | VECTOR_INVALID_TSS
                | VECTOR_SEGMENT_NP
                | VECTOR_STACK_FAULT
                | VECTOR_GP_FAULT
                | VECTOR_PAGE_FAULT
                | VECTOR_ALIGNMENT_CHECK
        )
    }
}
