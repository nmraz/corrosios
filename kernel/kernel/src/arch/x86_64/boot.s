.section .boottext, "ax"

.global boot_main
boot_main:
1:
    hlt
    jmp 1b
