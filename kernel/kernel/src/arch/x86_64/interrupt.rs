use core::arch::global_asm;

use paste::paste;

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
    panic!("exception {}", frame.vector);
}

unsafe fn handle_irq(frame: &InterruptFrame) {
    println!("IRQ {}", frame.vector);
}

#[no_mangle]
unsafe extern "C" fn handle_interrupt(frame: &InterruptFrame) {
    unsafe {
        if frame.vector < 32 {
            handle_exception(frame);
        } else {
            handle_irq(frame);
        }
    }
}

macro_rules! interrupt_stub {
    ($vector:literal, $name:ident) => {
        paste! {
            extern "C" {
                pub fn [<interrupt_vector_ $vector>]();
            }
            global_asm!(
                "
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
                has_error_code = const has_error_code!($name)
            );
        }
    };
}

macro_rules! has_error_code {
    // Double fault
    (df) => {
        1
    };

    // Invalid TSS
    (ts) => {
        1
    };

    // Segment not present
    (np) => {
        1
    };

    // Stack fault
    (ss) => {
        1
    };

    // General protection fault
    (gp) => {
        1
    };

    // Page fault
    (pf) => {
        1
    };

    // Alignment check exception
    (ac) => {
        1
    };

    // Control protection exception
    (cp) => {
        1
    };

    ($name:ident) => {
        0
    };
}

for_each_interrupt!(interrupt_stub);
global_asm!(include_str!("interrupt.s"));
