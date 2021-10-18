.macro func name
.type \name, @function
\name:
.endm

.macro func_end name
.size \name, . - \name
.endm

.macro global_func name
.global \name
func \name
.endm

.section .boottext, "ax"

global_func boot_main
1:
    hlt
    jmp 1b
func_end boot_main
