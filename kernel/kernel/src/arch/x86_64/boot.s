.macro pt_index reg level virt
    lea \reg, [\virt]
    shr \reg, \level * {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    and \reg, {PT_ENTRY_COUNT} - 1
.endm

.macro boottext_pt_index reg level
    pt_index \reg \level __boottext_start
.endm

.macro initial_kernel_pt_index reg level
    pt_index \reg \level __virt_start
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
    .quad GDT - {KERNEL_OFFSET}
.size early_gdtr, . - early_gdtr


.section .boot.bss, "aw", @nobits

.align {PAGE_SIZE}
.type boottext_pdpt, @object
boottext_pdpt:
    .skip {PAGE_SIZE}
.size boottext_pdpt, . - boottext_pdpt

.align {PAGE_SIZE}
.type boottext_pd, @object
boottext_pd:
    .skip {PAGE_SIZE}
.size boottext_pd, . - boottext_pd


.section .boot.text, "ax"

.global boot_main
.type boot_main, @function
boot_main:
    lea rsp, [boot_stack_top - {KERNEL_OFFSET}]

    # NOTE: We must avoid clobbering `rdi` here as it contains the physical
    # address of the data provided by the bootloader.

    # Initialize a temporary 8MiB identity mapping of the kernel so that
    # pivoting to our new page table doesn't cause an irrecoverable page fault.
    # NOTE: keep size in sync with check in linker script and `kernel_tables.rs`

    # Present, writable, executable
    lea rax, [boottext_pdpt + 0x3]
    boottext_pt_index rbx 3
    mov [KERNEL_PML4 - {KERNEL_OFFSET} + 8 * rbx], rax

    lea rax, [boottext_pd + 0x3]
    boottext_pt_index rbx 2
    mov [boottext_pdpt + 8 * rbx], rax

    lea rax, [__boottext_start]
    and rax, -0x200000
    # Present, writable, executable, large
    or rax, 0x83
    boottext_pt_index rbx 1

    # Map with 4 large 2MiB pages
    mov rcx, 4
.Lfill_low_pd:
    mov [boottext_pd + 8 * rbx], rax
    add rax, 0x200000
    add rbx, 1
    loop .Lfill_low_pd

    # Initialize kernel mapping at -2GiB

    lea rax, [KERNEL_PDPT - {KERNEL_OFFSET} + 0x3]
    initial_kernel_pt_index rbx 3
    mov [KERNEL_PML4 - {KERNEL_OFFSET} + 8 * rbx], rax

    lea rax, [KERNEL_PD - {KERNEL_OFFSET} + 0x3]
    initial_kernel_pt_index rbx 2
    mov [KERNEL_PDPT - {KERNEL_OFFSET} + 8 * rbx], rax

    # Compute number of aligned 2MiB ranges intersected by kernel
    lea rcx, [__virt_end + ({PAGE_SIZE} << {PT_LEVEL_SHIFT}) - 1]
    shr rcx, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    lea rax, [__virt_start]
    shr rax, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    sub rcx, rax

    lea rax, [KERNEL_PTS - {KERNEL_OFFSET} + 0x3]

    initial_kernel_pt_index rdx 1
    lea rsi, [KERNEL_PD - {KERNEL_OFFSET} + 8 * rdx]

.Lfill_kernel_pd:
    mov [rsi], rax
    add rsi, 8
    add rax, {PAGE_SIZE}
    loop .Lfill_kernel_pd

    lea rax, [__phys_start + 0x3]
    lea rbx, [__phys_end]

    initial_kernel_pt_index rdx 0
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

    # Remove boot code mapping
    mov qword ptr [KERNEL_PML4], 0

    # Flush TLB
    mov rax, cr3
    mov cr3, rax

    lea rsp, [boot_stack_top]
    jmp kernel_main
.size high_entry, . - high_entry
