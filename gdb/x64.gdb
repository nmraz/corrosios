set disassembly-flavor intel
thbreak kernel_main
c
b panic_fmt
