ENTRY(boot_main)

SECTIONS {
    . = 0;

    __boottext_start = .;

    .boot.text : {
        *(.boot.text)
    }

    . = ALIGN(4K);

    .boot.data : {
        *(.boot.rodata)
        *(.boot.data)
    }

    . = ALIGN(4K);

    .boot.bss : {
        *(.boot.bss)
    }

    . = ALIGN(4K);

    __phys_start = .;
    . = 0xFFFFFFFF80000000; /* -2GiB */
    __virt_start = .;

    __code_start = .;
    .text : AT(ADDR(.text) - __virt_start + __phys_start) {
        *(.text*)
    }
    . = ALIGN(4K);
    __code_end = .;

    __rodata_start = .;
    .rodata : AT(ADDR(.rodata) - __virt_start + __phys_start) {
        *(.rodata*)
    }
    . = ALIGN(4K);
    __rodata_end = .;

    __data_start = .;
    .data : AT(ADDR(.data) - __virt_start + __phys_start) {
        *(.data*)
    }
    . = ALIGN(4K);
    __data_end = .;

    __bss_start = .;
    .bss : AT(ADDR(.bss) - __virt_start + __phys_start) {
        *(COMMON)
        *(.bss*)
    }
    . = ALIGN(4K);
    __bss_end = .;

    __virt_end = .;
    __phys_end = . - __virt_start + __phys_start;

    /* Keep in sync with size reserved in `kernel_tables.rs` and early mapping */
    ASSERT(__phys_end - __boottext_start < 8M, "Kernel too large")
}
