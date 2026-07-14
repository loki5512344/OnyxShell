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
mod features;
mod io;
mod path;
mod pipeline;
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

        // History expansion: `!!`, `!N`, `!-N` are replaced with the
        // corresponding history entry before execution. The expanded
        // line is what gets tokenized and run.
        let expanded = unsafe { features::history_expand(raw) };
        let expanded_slice = expanded.as_slice();
        // Push the (expanded) line into history for future reference.
        unsafe { features::history_push(expanded_slice); }

        // Check if the line contains a pipe `|` or redirect `>` / `<`.
        let has_pipe_or_redirect = expanded_slice.iter().any(|&b| b == b'|' || b == b'>' || b == b'<');
        if has_pipe_or_redirect {
            // For pipelines we don't glob-expand (would need to expand per-segment).
            // Copy into a static buffer so pipeline::parse can take a &[u8].
            static mut G_EXPANDED: [u8; LINE_MAX] = [0u8; LINE_MAX];
            let n = expanded_slice.len().min(LINE_MAX - 1);
            unsafe {
                for i in 0..n {
                    G_EXPANDED[i] = expanded_slice[i];
                }
                G_EXPANDED[n] = 0;
                let p = pipeline::parse(&G_EXPANDED[..n]);
                pipeline::execute(&G_EXPANDED[..n], &p);
            }
            continue;
        }

        // Tokenize into arguments.
        let ntok = io::tokenize(expanded_slice, unsafe { &mut G_TOKEN_OFFSETS });
        if ntok == 0 {
            continue;
        }

        // Glob expansion: expand wildcards (`*`, `?`, `[...]`) in each
        // token into a list of matching filesystem paths. Tokens without
        // glob characters pass through unchanged.
        static mut G_EXPANDED_ARGS: [[u8; 128]; 32] = [[0u8; 128]; 32];
        static mut G_ARGS: [&[u8]; 32] = [&[]; 32];
        let mut n_args = 0usize;
        unsafe {
            for i in 0..ntok {
                let (off, len) = G_TOKEN_OFFSETS[i];
                let tok = &expanded_slice[off..off + len];
                let expansions = features::glob_expand(tok);
                for j in 0..expansions.len() {
                    if n_args >= 32 {
                        break;
                    }
                    let exp = expansions[j];
                    // Find NUL terminator (or use full length).
                    let mut elen = 0;
                    while elen < 127 && exp[elen] != 0 {
                        elen += 1;
                    }
                    G_EXPANDED_ARGS[n_args] = exp;
                    G_ARGS[n_args] = &G_EXPANDED_ARGS[n_args][..elen];
                    n_args += 1;
                }
                if n_args >= 32 {
                    break;
                }
            }
        }

        // Dispatch to the command handler.
        commands::dispatch(unsafe { &G_ARGS[..n_args] });
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
