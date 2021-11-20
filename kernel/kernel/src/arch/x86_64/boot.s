.bss

.set BOOT_STACK_SIZE, 0x4000 # 16K
.set BOOT_STACK_ALIGN, 0x10 # As per ABI

.align BOOT_STACK_ALIGN
.type boot_stack, @object
boot_stack:
    .skip BOOT_STACK_SIZE
.size boot_stack, . - boot_stack

.section .boot.rodata, "a"

.type early_gdtr, @object
early_gdtr:
    .word {GDT_SIZE}
    .long GDT - {KERNEL_OFFSET}
.size early_gdtr, . - early_gdtr

.section .boot.text, "ax"

.global boot_main
.type boot_main, @function
boot_main:
1:
    hlt
    jmp 1b
.size boot_main, . - boot_main
