ENTRY(start)

kernel_offset = 0xffffffff80000000;

. = kernel_offset;

SECTIONS {
    __kernel_code_start = .;
    .text ALIGN(4K) :
    {
        KEEP(*(.text*))
    }
    __kernel_code_end = .;
    __kernel_data_start = .;
    .rodata ALIGN(4K) :
    {
        KEEP(*(.rodata*))
    }

    .data ALIGN(4K) :
    {
        KEEP(*(.data*))
    }

    .bss ALIGN(4K) :
    {
        KEEP(*(COMMON))
        KEEP(*(.bss*))
    }

    .tdata ALIGN(4K) :
    {
        __tdata_start = .;
        KEEP(*(COMMON))
        KEEP(*(.tdata*))
        __tdata_end = .;
    }

    .tbss ALIGN(4K) :
    {
        __tbss_start = .;
        KEEP(*(COMMON))
        KEEP(*(.tbss*))
        __tbss_end = .;
    }
    __kernel_data_end = .;
}
