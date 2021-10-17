.section .boottext, "ax"

boot_main:
1:
    hlt
    jmp 1b
