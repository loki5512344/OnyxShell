//! OnyxShell (`/bin/osh`) — user-space shell for OnyxOS.
//!
//! This is a freestanding RISC-V 64-bit binary compiled with
//! `riscv64gc-unknown-none-elf` and converted to OnyxExec v2 format
//! via `elf2onx`. It runs in ring 1 (root) when launched by the
//! auto-login flow, or ring 2 (user) when launched after a regular
//! user login.
//!
//! ## Built-in commands
//!
//! `ls`, `cat`, `rm`, `cd`, `cp`, `mv`, `mkdir`, `touch`, `stat`,
//! `pwd`, `echo`, `whoami`, `uname`, `date`, `clear`, `help`, `exit`,
//! `exec`, `run`, `ver`
//!
//! ## Privilege model
//!
//! File-mutation commands (`rm`, `mkdir`, `cp`, `mv`, `touch`) require
//! root (ring 1). The default first-boot auto-login is root, so all
//! commands work out of the box. Regular users (ring 2) will get
//! `EPERM` from these commands.

#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn, non_snake_case, clippy::missing_safety_doc, static_mut_refs)]

use core::arch::asm;

mod commands;
mod io;
mod path;
mod syscalls;

/// Shell version banner (printed on startup).
const VERSION_BANNER: &str = "OnyxShell v0.3.0 (built-in commands)\n";

/// Shell prompt. Printed before each line of input.
const PROMPT: &str = "osh$ ";

/// Maximum input line length.
const LINE_MAX: usize = 256;

/// ── Static buffers ──────────────────────────────────────────────────────
/// We use static mutable buffers instead of stack-allocated ones.
/// This guarantees the buffers are in .bss (always mapped RW) and at
/// valid user addresses, eliminating potential stack-related issues
/// where user_ptr_ok() might reject a stack address.
///
/// Safety: There is only one shell process, so there are no reentrancy
/// or aliasing concerns.
static mut G_LINE: [u8; LINE_MAX] = [0u8; LINE_MAX];
static mut G_TOKEN_OFFSETS: [(usize, usize); commands::MAX_ARGS] = [(0usize, 0usize); commands::MAX_ARGS];

/// Entry point — called by the kernel's OnyxExec loader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    // Print version banner.
    syscalls::write(1, VERSION_BANNER.as_ptr(), VERSION_BANNER.len());

    // Main read-eval-print loop.
    loop {
        // Print prompt.
        syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());

        // Read a line from stdin. The kernel handles line editing
        // (echo, backspace) internally for fd 0.
        let n = io::read_line(unsafe { &mut G_LINE });
        if n == 0 {
            // read_line returned 0 — either an error or empty line.
            // Silently continue to the next prompt.
            continue;
        }

        // Strip trailing newline / carriage return / NUL.
        let mut end = n;
        while end > 0 && (G_LINE[end - 1] == b'\n' || G_LINE[end - 1] == b'\r' || G_LINE[end - 1] == 0) {
            end -= 1;
        }

        // Skip empty lines.
        if end == 0 {
            continue;
        }

        let raw = unsafe { &G_LINE[..end] };

        // Tokenize into arguments.
        let ntok = io::tokenize(raw, unsafe { &mut G_TOKEN_OFFSETS });
        if ntok == 0 {
            continue;
        }

        // Build a slice of argument slices for the dispatcher.
        // We use a static array to avoid stack allocation.
        static mut G_ARGS: [&[u8]; commands::MAX_ARGS] = [&[]; commands::MAX_ARGS];
        unsafe {
            for i in 0..ntok {
                let (off, len) = G_TOKEN_OFFSETS[i];
                G_ARGS[i] = &raw[off..off + len];
            }
        }

        // Dispatch to the command handler.
        commands::dispatch(unsafe { &G_ARGS[..ntok] });
    }
}

/// Panic handler — print a message and halt.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        io::write_str("osh: internal panic — halting\n");
        loop {
            asm!("wfi");
        }
    }
}
