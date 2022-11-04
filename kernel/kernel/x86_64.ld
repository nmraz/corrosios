ENTRY(boot_main)

PHDRS {
    boot PT_LOAD;
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
}

SECTIONS {
    . = 0;

    __boottext_start = .;

    .boot : {
        *(.boot.text)
        *(.boot.rodata)
        *(.boot.data)
    } :boot

    .boot.bss : {
        *(.boot.bss)
    } :boot

    . = ALIGN(4K);

    __phys_start = .;
    . = 0xFFFFFFFF80000000; /* -2GiB */
    __virt_start = .;

    __code_start = .;
    .text : AT(ADDR(.text) - __virt_start + __phys_start) {
        *(.text*)
    } :text
    . = ALIGN(4K);
    __code_end = .;

    __rodata_start = .;
    .rodata : AT(ADDR(.rodata) - __virt_start + __phys_start) {
        *(.rodata*)
    } :rodata
    . = ALIGN(4K);
    __rodata_end = .;

    __data_start = .;
    .data : AT(ADDR(.data) - __virt_start + __phys_start) {
        *(.data*)
    } :data

    .bss : AT(ADDR(.bss) - __virt_start + __phys_start) {
        *(COMMON)
        *(.bss*)
    } :data
    . = ALIGN(4K);
    __data_end = .;

    __virt_end = .;
    __phys_end = . - __virt_start + __phys_start;

    /* Keep in sync with `KERNEL_MAX` in `mmu.rs` and early mapping */
    ASSERT(__phys_end - __boottext_start < 8M, "Kernel too large")
}
