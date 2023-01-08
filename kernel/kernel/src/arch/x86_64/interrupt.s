.type interrupt_entry_common, @function
interrupt_entry_common:
    cld

    push r15
    push r14
    push r13
    push r12
    push r11
    push r10
    push r9
    push r8
    push rbp
    push rdi
    push rsi
    push rdx
    push rcx
    push rbx
    push rax

    mov rdi, rsp

    // Note: our stack is now exactly 16-byte aligned: 6 qwords pushed by the CPU (including error
    // code which may be pushed manually), another qword for vector number and 15 qwords for saved
    // registers, totalling 22 qwords. If the number of pushes changes, the stack may have to be
    // realigned to a 16-byte boundary here before calling `handle_interrupt`, as per ABI.
    call handle_interrupt

    pop rax
    pop rbx
    pop rcx
    pop rdx
    pop rsi
    pop rdi
    pop rbp
    pop r8
    pop r9
    pop r10
    pop r11
    pop r12
    pop r13
    pop r14
    pop r15

    // Pop vector number and error code
    add rsp, 0x10
    iretq
.size interrupt_entry_common, . - interrupt_entry_common
