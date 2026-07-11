OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS {
    /* OnyxExec v2 loads user binaries at 0x10000 (USER_BASE).
       The kernel's onx::load() maps the code segment starting here. */
    . = 0x10000;

    .text : ALIGN(4) {
        KEEP(*(.text.start))
        *(.text .text.*)
    }

    .rodata : ALIGN(8) {
        *(.rodata .srodata .rodata.* .srodata.*)
    }

    .data : ALIGN(8) {
        *(.data .sdata .data.* .sdata.*)
    }

    /* Page-align .bss so it does not share a page with .rodata.
       OnyxKernel maps pages with specific permissions; if .bss and
       .rodata land on the same page, the first write to .bss will
       page-fault because the page was mapped read-only.
       (From patch 04-linker-bss-page-align.patch.) */
    . = ALIGN(0x1000);
    __bss_start = .;
    .bss : ALIGN(8) {
        *(.bss .sbss .bss.* .sbss.* COMMON)
    }
    . = ALIGN(8);
    __bss_end = .;

    /DISCARD/ : {
        *(.note .note.*)
        *(.comment)
        *(.eh_frame .eh_frame.*)
        *(.riscv.attributes)
    }
}
