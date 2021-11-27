.bss

.set BOOT_STACK_SIZE, 0x4000 # 16K
.set BOOT_STACK_ALIGN, 0x10 # As per ABI

.align BOOT_STACK_ALIGN
.type boot_stack, @object
boot_stack:
    .skip BOOT_STACK_SIZE
.size boot_stack, . - boot_stack
boot_stack_top:

.section .boot.rodata, "a"

.type early_gdtr, @object
early_gdtr:
    .word {GDT_SIZE}
    .long GDT - {KERNEL_OFFSET}
.size early_gdtr, . - early_gdtr

.align {PAGE_SIZE}
.type early_low_pdpt, @object
early_low_pdpt:
    .skip {PAGE_SIZE}
.size early_low_pdpt, . - early_low_pdpt

.align {PAGE_SIZE}
.type early_low_pd, @object
early_low_pd:
    .skip {PAGE_SIZE}
.size early_low_pd, . - early_low_pd

.section .boot.text, "ax"

.global boot_main
.type boot_main, @function
boot_main:
    lea rsp, [boot_stack_top - {KERNEL_OFFSET}]

    # Initialize early low 1GiB mapping

    # Present, writable, executable
    lea rax, [early_low_pdpt + 0x3]
    mov [KERNEL_PML4 - {KERNEL_OFFSET}], rax
    lea rax, [early_low_pd + 0x3]
    mov [early_low_pdpt], rax
    # Present, writable, executable, huge
    mov qword ptr [early_low_pd], 0x83

    # Initialize kernel mapping at -2GiB

    lea rax, [KERNEL_PDPT - {KERNEL_OFFSET} + 0x3]
    mov [KERNEL_PML4 - {KERNEL_OFFSET} + 0x1ff], rax
    lea rax, [KERNEL_PD - {KERNEL_OFFSET} + 0x3]
    mov [KERNEL_PDPT - {KERNEL_OFFSET} + 0x1ff], rax

    lea rax, [__phys_start + 0x3]
    lea rbx, [__phys_end]

    # Find initial PD index
    lea rdx, [__virt_start]
    shr rdx, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    and rdx, {PT_ENTRY_COUNT} - 1

    lea rdi, [KERNEL_PD - {KERNEL_OFFSET} + rdx]

.Lfill_kernel_pd:
    mov [rdi], rax
    add rdi, 8 # Entry size
    add rax, {PAGE_SIZE} << {PT_LEVEL_SHIFT}
    cmp rax, rbx
    jl .Lfill_kernel_pd

    lea rax, [__phys_start + 0x3]

    # Find initial PT index
    lea rdx, [__virt_start]
    shr rdx, {PAGE_SHIFT}
    and rdx, {PT_ENTRY_COUNT} - 1

    lea rdi, [KERNEL_PTS - {KERNEL_OFFSET} + rdx]

.Lfill_kernel_pts:
    mov [rdi], rax
    add rdi, 8
    add rax, {PAGE_SIZE}
    cmp rax, rbx
    jl .Lfill_kernel_pts

    lea rax, [KERNEL_PML4 - {KERNEL_OFFSET}]
    mov cr3, rax
    lgdt [early_gdtr]

    push {KERNEL_CS_SELECTOR}
    lea rax, [high_entry]
    push rax
    retf
.size boot_main, . - boot_main

.text

.type high_entry, @function
high_entry:
1:
    hlt
    jmp 1b
.size high_entry, . - high_entry
