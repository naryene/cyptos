OUTPUT_ARCH(riscv)
ENTRY(_start)

MEMORY
{
    /* Main RAM: 128MB starting at 0x80000000 */
    /* Kernel starts at 0x80200000 to leave room for SBI firmware */
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 128M
}


KERNEL_STACK_SIZE = 64K;
KERNEL_HEAP_SIZE = 16M;


SECTIONS
{
    . = ORIGIN(RAM);

    /* Kernel code section */
    .text : ALIGN(4K)
    {
        _stext = .;
        *(.text.entry)      /* Entry point must be first */
        *(.text.trap)       /* Trap vector aligned */
        *(.text .text.*)
        _etext = .;
    } > RAM

    /* User-mode task code — placed separately to support future permission control */
    .user_text : ALIGN(4K)
    {
        _user_text_start = .;
        *(.user_text .user_text.*)
        . = ALIGN(4K);
        _user_text_end = .;
    } > RAM

    /* Read-only data */
    .rodata : ALIGN(4K)
    {
        _srodata = .;
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        . = ALIGN(8);
        __cyptos_tests_start = .;
        KEEP(*(.cyptos_tests))
        __cyptos_tests_end = .;
        _erodata = .;
    } > RAM

    /* Initialized data */
    .data : ALIGN(4K)
    {
        _sdata = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        _edata = .;
    } > RAM

    /* Uninitialized data (BSS) */
    .bss : ALIGN(4K)
    {
        _sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        _ebss = .;
    } > RAM

    /* Kernel stack */
    . = ALIGN(4K);
    _sstack = .;
    . += KERNEL_STACK_SIZE;
    _estack = .;

    /* Kernel heap */
    . = ALIGN(4K);
    _sheap = .;
    . += KERNEL_HEAP_SIZE;
    _eheap = .;

    /* End of kernel image */
    . = ALIGN(4K);
    _end = .;

    /* Discard unwanted sections */
    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
        *(.note.*)
        *(.comment)
    }
}

/* Export symbols for Rust code */
PROVIDE(_stack_top = _estack);
PROVIDE(_heap_start = _sheap);
PROVIDE(_heap_end = _eheap);
PROVIDE(__global_pointer$ = _sdata + 0x800);
