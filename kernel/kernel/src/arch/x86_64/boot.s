.macro initial_kernel_pt_index reg level
    lea \reg, [__virt_start]
    shr \reg, (\level - 1) * {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    and \reg, {PT_ENTRY_COUNT} - 1
.endm


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
    .word {GDT_SIZE} * 8 - 1
    .long GDT - {KERNEL_OFFSET}
.size early_gdtr, . - early_gdtr


.section .boot.bss, "aw", @nobits

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

    # NOTE: We must avoid clobbering `rdi` here as it contains the physical
    # address of the data provided by the bootloader.

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
    initial_kernel_pt_index rbx 4
    mov [KERNEL_PML4 - {KERNEL_OFFSET} + 8 * rbx], rax

    lea rax, [KERNEL_PD - {KERNEL_OFFSET} + 0x3]
    initial_kernel_pt_index rbx 3
    mov [KERNEL_PDPT - {KERNEL_OFFSET} + 8 * rbx], rax

    # Compute number of 2MiB ranges necessary to cover kernel
    lea rcx, [__phys_end + ({PAGE_SIZE} << {PT_LEVEL_SHIFT}) - 1]
    lea rax, [__phys_start]
    sub rcx, rax
    shr rcx, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}

    # Find offset in `KERNEL_PTS` of first page table needed to cover kernel,
    # assuming that they start covering at physical address 0. This is
    # effectively a division by 2MiB followed by a multiplication by 4KiB.
    lea rax, [__phys_start]
    shr rax, {PT_LEVEL_SHIFT}
    and rax, -{PAGE_SIZE}

    lea rax, [KERNEL_PTS - {KERNEL_OFFSET} + rax]
    or rax, 3

    initial_kernel_pt_index rdx 2
    lea rsi, [KERNEL_PD - {KERNEL_OFFSET} + 8 * rdx]

.Lfill_kernel_pd:
    mov [rsi], rax
    add rsi, 8
    add rax, {PAGE_SIZE}
    loop .Lfill_kernel_pd

    lea rax, [__phys_start + 0x3]
    lea rbx, [__phys_end]

    initial_kernel_pt_index rdx 1
    lea rsi, [KERNEL_PTS - {KERNEL_OFFSET} + 8 * rdx]

.Lfill_kernel_pts:
    mov [rsi], rax
    add rsi, 8
    add rax, {PAGE_SIZE}
    cmp rax, rbx
    jl .Lfill_kernel_pts

    lea rax, [KERNEL_PML4 - {KERNEL_OFFSET}]
    mov cr3, rax
    lgdt [early_gdtr]

    push {KERNEL_CS_SELECTOR}
    lea rax, [high_entry]
    push rax
    retfq
.size boot_main, . - boot_main


.text

.type high_entry, @function
high_entry:
    # NOTE: Avoid clobbering `rdi`.

    xor eax, eax
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    lea rsp, [boot_stack_top]
    jmp kernel_main
.size high_entry, . - high_entry
