ENTRY(boot_main)

SECTIONS {
    . = 1M;

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

    __bootseg_end = .;

    __phys_start = .;
    . += 0xFFFFFFFF80000000; /* -2GiB */
    __virt_start = .;

    __code_start = .;
    .text : AT(ADDR(.text) - __virt_start + __bootseg_end) {
        *(.text*)
    }
    . = ALIGN(4K);
    __code_end = .;

    __rodata_start = .;
    .rodata : AT(ADDR(.rodata) - __virt_start + __bootseg_end) {
        *(.rodata*)
        /* For some reason, a GOT is created for certain global variable
           accesses; make sure it gets included. */
        *(.got)
    }
    . = ALIGN(4K);
    __rodata_end = .;

    __data_start = .;
    .data : AT(ADDR(.data) - __virt_start + __bootseg_end) {
        *(.data*)
    }
    . = ALIGN(4K);
    __data_end = .;

    __bss_start = .;
    .bss : AT(ADDR(.bss) - __virt_start + __bootseg_end) {
        *(COMMON)
        *(.bss*)
    }
    . = ALIGN(4K);
    __bss_end = .;

    __virt_end = .;
    __phys_end = . - __virt_start + __bootseg_end;

    /* Keep in sync with size reserved in `kernel_tables.rs` and early mapping
       created in `boot.s` */
    ASSERT(__phys_end < 8M, "Kernel too large")
}
