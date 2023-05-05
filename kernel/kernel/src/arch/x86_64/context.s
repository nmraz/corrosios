.type do_context_switch, @function
do_context_switch:
    // Note: this must be consistent with the definition of `InactiveKernelFrame`.
    // `rip` will have already been pushed as the return address when calling this function.
    push r15
    push r14
    push r13
    push r12
    push rbp
    push rbx

    mov [rdi], rsp
.type do_context_set, @function
do_context_set:
    mov rsp, rsi

    pop rbx
    pop rbp
    pop r12
    pop r13
    pop r14
    pop r15

    ret
.size do_context_set, . - do_context_set
.size do_context_switch, . - do_context_switch

.type early_thread_start, @function
early_thread_start:
    // Clean up some state because we can. Note that only caller-saved registers need to be cleared
    // here, as everything else should have been cleaned up by the context switch itself.
    xor rax, rax
    xor rcx, rcx
    xor rdx, rdx
    xor rsi, rsi
    // `rdi` will be set below
    xor r8, r8
    xor r9, r9
    xor r10, r10
    xor r11, r11

    // Both the function to call and its argument are passed via the stack.
    pop rdi
    ret
.size early_thread_start, . - early_thread_start
