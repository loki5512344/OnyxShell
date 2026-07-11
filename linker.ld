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

    /* Page-align .data so it does NOT share a page with .rodata.
       OnyxKernel maps each LOAD segment with its own permissions.
       If .data and .rodata land on the same page, the kernel maps
       the page as read-only (from .rodata) and the first write to
       .data page-faults. Page-aligning ensures .data gets its own
       page mapped read-write. */
    . = ALIGN(0x1000);
    .data : ALIGN(8) {
        *(.data .sdata .data.* .sdata.*)
    }
    . = ALIGN(8);

    /* Page-align .bss so it does not share a page with .data.
       OnyxKernel maps pages with specific permissions; if .bss and
       .data land on the same page, the first write to .bss will
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
