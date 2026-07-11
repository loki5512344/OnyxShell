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
//!
//! ## Compilation
//!
//! ```sh
//! cargo build --release
//! # Convert ELF → OnyxExec v2:
//! elf2onx --compress target/riscv64gc-unknown-none-elf/release/onyx-osh osh.onx
//! ```
//!
//! The resulting `osh.onx` is placed at `/bin/osh` in the OnyxFS
//! disk image.

#![no_std]
#![no_main]
#![allow(unsafe_op_in_unsafe_fn, non_snake_case, clippy::missing_safety_doc)]

use core::arch::asm;

mod commands;
mod io;
mod path;
mod syscalls;

/// Shell version banner (printed on startup).
const VERSION_BANNER: &str = "OnyxShell v0.2.0 (built-in commands)\n";

/// Shell prompt. Printed before each line of input.
const PROMPT: &str = "osh$ ";

/// Maximum input line length.
const LINE_MAX: usize = 256;

/// Entry point — called by the kernel's OnyxExec loader.
///
/// The kernel sets `a0 = argc` and `a1 = argv_ptr` before jumping here,
/// but the shell ignores them (login execs `/bin/osh` with no args).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    // Print version banner.
    syscalls::write(1, VERSION_BANNER.as_ptr(), VERSION_BANNER.len());

    // Main read-eval-print loop.
    let mut line = [0u8; LINE_MAX];
    let mut token_offsets = [(0usize, 0usize); commands::MAX_ARGS];

    loop {
        // Print prompt.
        syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());

        // Read a line from stdin. The kernel handles line editing
        // (echo, backspace) internally for fd 0.
        let n = io::read_line(&mut line);
        if n == 0 {
            continue;
        }

        // Strip trailing newline / carriage return / NUL.
        let mut end = n;
        while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r' || line[end - 1] == 0) {
            end -= 1;
        }

        // Skip empty lines.
        if end == 0 {
            continue;
        }

        let raw = &line[..end];

        // Tokenize into arguments.
        let ntok = io::tokenize(raw, &mut token_offsets);
        if ntok == 0 {
            continue;
        }

        // Build a slice of argument slices for the dispatcher.
        let mut args: [&[u8]; commands::MAX_ARGS] = [&[]; commands::MAX_ARGS];
        for i in 0..ntok {
            let (off, len) = token_offsets[i];
            args[i] = &raw[off..off + len];
        }

        // Dispatch to the command handler.
        commands::dispatch(&args[..ntok]);
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
