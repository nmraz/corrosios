set disassembly-flavor intel
thbreak kernel::kernel_main
c
b rust_begin_unwind
