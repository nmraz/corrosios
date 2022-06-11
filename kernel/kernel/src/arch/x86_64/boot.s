.macro pt_index reg, level, virt
    lea \reg, [\virt]
    shr \reg, \level * {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    and \reg, {PT_ENTRY_COUNT} - 1
.endm

.macro boottext_pt_index reg, level
    pt_index \reg, \level, rip + __boottext_start
.endm

.macro initial_kernel_pt_index reg, level
    pt_index \reg, \level, __virt_start
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
    .quad GDT
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
    # NOTE: We must avoid clobbering `rdi` for the duration of this function as
    # it contains the physical address of the data provided by the bootloader.

    # The kernel is physically relocatable, so we must stick to pure PIC
    # here until the kernel is mapped to its (constant) virtual address. For
    # the remainder of this function, `r8` will hold the physical address of the
    # kernel (excluding early boot code) and `r9` will hold the delta between
    # the kernel's physical and virtual addresses.

    lea r8, [rip + __phys_start]
    lea r9, [__virt_start]
    neg r9
    add r9, r8

    lea rsp, [boot_stack_top + r9]

    # Initialize a temporary 10MiB identity mapping of the kernel so that
    # pivoting to our new page table doesn't cause an irrecoverable page fault.

    # NOTE: keep size in sync with check in linker script and
    # `kernel_tables.rs`; we intentionally use an additional 2MiB entry here
    # in case the kernel isn't physically aligned to a 2MiB boundary.

    # Present, writable, executable
    lea rax, [boottext_pdpt + 0x3]
    boottext_pt_index rbx, 3
    mov [KERNEL_PML4 + r9 + 8 * rbx], rax

    lea rax, [boottext_pd + 0x3]
    boottext_pt_index rbx, 2
    mov [boottext_pdpt + 8 * rbx], rax

    lea rax, [__boottext_start]
    and rax, -({PAGE_SIZE} << {PT_LEVEL_SHIFT})
    # Present, writable, executable, large
    or rax, 0x83
    boottext_pt_index rbx, 1

    # Map with 5 large 2MiB pages
    mov rcx, 5
.Lfill_boottext_pd:
    mov [boottext_pd + 8 * rbx], rax
    add rax, {PAGE_SIZE} << {PT_LEVEL_SHIFT}
    add rbx, 1
    loop .Lfill_boottext_pd

    # Initialize kernel mapping at -2GiB

    lea rax, [KERNEL_PDPT + r9 + 0x3]
    initial_kernel_pt_index rbx, 3
    mov [KERNEL_PML4 + r9 + 8 * rbx], rax

    lea rax, [KERNEL_PD + r9 + 0x3]
    initial_kernel_pt_index rbx, 2
    mov [KERNEL_PDPT + r9 + 8 * rbx], rax

    # Compute number of aligned 2MiB ranges intersected by kernel
    lea rcx, [__virt_end + ({PAGE_SIZE} << {PT_LEVEL_SHIFT}) - 1]
    shr rcx, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    lea rax, [__virt_start]
    shr rax, {PT_LEVEL_SHIFT} + {PAGE_SHIFT}
    sub rcx, rax

    lea rax, [KERNEL_PTS + r9 + 0x3]

    initial_kernel_pt_index rdx, 1
    lea rsi, [KERNEL_PD + r9 + 8 * rdx]

.Lfill_kernel_pd:
    mov [rsi], rax
    add rsi, 8
    add rax, {PAGE_SIZE}
    loop .Lfill_kernel_pd

    lea rax, [__phys_start + 0x3]
    lea rbx, [__phys_end]

    initial_kernel_pt_index rdx, 0
    lea rsi, [KERNEL_PTS + r9 + 8 * rdx]

.Lfill_kernel_pts:
    mov [rsi], rax
    add rsi, 8
    add rax, {PAGE_SIZE}
    cmp rax, rbx
    jl .Lfill_kernel_pts

    lea rax, [KERNEL_PML4 + r9]
    mov cr3, rax

    lgdt [early_gdtr]

    mov rsi, rdi
    mov rdi, r8
    boottext_pt_index rdx, 3

    push {KERNEL_CS_SELECTOR}
    lea rax, [high_entry]
    push rax
    retfq
.size boot_main, . - boot_main


.text

.type high_entry, @function
high_entry:
    # Parameters:
    # 1 (rdi) - Kernel physical address
    # 2 (rsi) - Bootdata physical address
    # 3 (rdx) - Level-3 page table index of early boot code mapping

    xor eax, eax
    mov ss, ax
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    # Remove boot code mapping
    mov qword ptr [KERNEL_PML4 + 8 * rdx], 0

    # Flush TLB
    mov rax, cr3
    mov cr3, rax

    lea rsp, [boot_stack_top]

    # NOTE: parameters 1 and 2 carry over into `kernel_main`. We perform a
    # `call` here to ensure that `rsp - 8` is 16-byte aligned upon function
    # entry, as mandated by the ABI.
    call kernel_main
.size high_entry, . - high_entry
