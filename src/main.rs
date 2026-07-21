//! OnyxShell (`/bin/osh`) — user-space shell for OnyxOS.
//!
//! This version uses raw terminal mode (TIOCSRAW) to enable:
//!   - Tab completion (file-system + built-in command names)
//!   - Arrow-key history navigation (Up/Down)
//!   - In-line editing (Left/Right cursor movement, Backspace)
//!
//! The kernel's cooked (line-edited) mode is used as a fallback if
//! enabling raw mode fails (e.g. on an older kernel without the
//! TIOCSRAW ioctl).

#![no_std]
#![no_main]
#![allow(
    unsafe_op_in_unsafe_fn,
    non_snake_case,
    clippy::missing_safety_doc,
    static_mut_refs
)]

use core::arch::asm;

mod commands;
mod features;
mod io;
mod path;
mod pipeline;
mod syscalls;

/// Shell version banner (printed on startup).
const VERSION_BANNER: &str = "OnyxShell v0.4.0 (raw mode: tab completion + arrow history)\n";

/// Shell prompt. Printed before each line of input.
const PROMPT: &str = "osh$ ";

/// Maximum input line length.
const LINE_MAX: usize = 256;

/// ── Static buffers ──────────────────────────────────────────────────────
static mut G_LINE: [u8; LINE_MAX] = [0u8; LINE_MAX];
static mut G_TOKEN_OFFSETS: [(usize, usize); 16] = [(0usize, 0usize); 16];

/// Entry point — called by the kernel's OnyxExec loader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    syscalls::write(1, VERSION_BANNER.as_ptr(), VERSION_BANNER.len());

    features::env_init();

    // Try to enable raw terminal mode for tab completion + arrow history.
    // If the kernel doesn't support TIOCSRAW (returns ENOSYS), we fall
    // back to cooked (line-edited) mode.
    let raw_ok = syscalls::enable_raw_mode() == 0;

    if raw_ok {
        raw_mode_repl();
    } else {
        cooked_mode_repl();
    }
}

// ── Raw-mode REPL ───────────────────────────────────────────────────────
//
// In raw mode we read byte-at-a-time and handle:
//   - Tab (0x09): tab completion
//   - Up (ESC [ A) / Down (ESC [ B): history navigation
//   - Left (ESC [ D) / Right (ESC [ C): cursor movement (TODO)
//   - Backspace (0x7F/0x08): delete char before cursor
//   - Enter (\r/\n): submit line
//   - Ctrl-C (0x03): cancel line
//   - Ctrl-D (0x04): EOF (exit shell)
//   - Printable chars: insert at cursor

unsafe fn raw_mode_repl() -> ! {
    let mut line: [u8; LINE_MAX] = [0u8; LINE_MAX];
    let mut line_len: usize = 0;
    let mut cursor: usize = 0;
    let mut rx_buf = [0u8; 16];

    loop {
        // Print prompt.
        syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());
        line_len = 0;
        cursor = 0;
        features::nav_reset();

        'line_loop: loop {
            let n = syscalls::read(0, rx_buf.as_mut_ptr(), rx_buf.len() as u64);
            if n <= 0 {
                continue;
            }
            let n = n as usize;
            let mut i = 0;
            while i < n {
                let b = rx_buf[i];

                // ESC sequence (arrow keys, etc.)
                if b == 0x1B && i + 2 < n {
                    if rx_buf[i + 1] == b'[' {
                        match rx_buf[i + 2] {
                            b'A' => {
                                // Up — history previous
                                if let Some(entry) = features::nav_up() {
                                    // Clear current line, replace with entry.
                                    clear_line(&line[..line_len], cursor);
                                    let cn = entry.len().min(LINE_MAX - 1);
                                    for j in 0..cn {
                                        line[j] = entry[j];
                                    }
                                    line_len = cn;
                                    cursor = cn;
                                    syscalls::write(1, line.as_ptr(), line_len);
                                }
                                i += 3;
                                continue;
                            }
                            b'B' => {
                                // Down — history next
                                match features::nav_down() {
                                    Some(entry) => {
                                        clear_line(&line[..line_len], cursor);
                                        let cn = entry.len().min(LINE_MAX - 1);
                                        for j in 0..cn {
                                            line[j] = entry[j];
                                        }
                                        line_len = cn;
                                        cursor = cn;
                                        syscalls::write(1, line.as_ptr(), line_len);
                                    }
                                    None => {
                                        // Back to current (empty) line.
                                        clear_line(&line[..line_len], cursor);
                                        line_len = 0;
                                        cursor = 0;
                                    }
                                }
                                i += 3;
                                continue;
                            }
                            b'C' => {
                                // Right — move cursor right (TODO)
                                if cursor < line_len {
                                    cursor += 1;
                                    // Move cursor right: ESC [ C
                                    syscalls::write(1, b"\x1B[C".as_ptr(), 3);
                                }
                                i += 3;
                                continue;
                            }
                            b'D' => {
                                // Left — move cursor left
                                if cursor > 0 {
                                    cursor -= 1;
                                    syscalls::write(1, b"\x1B[D".as_ptr(), 3);
                                }
                                i += 3;
                                continue;
                            }
                            _ => {}
                        }
                    }
                    // Unknown ESC sequence — skip the ESC.
                    i += 1;
                    continue;
                }

                match b {
                    b'\r' | b'\n' => {
                        // Submit line.
                        syscalls::write(1, b"\r\n".as_ptr(), 2);
                        break 'line_loop;
                    }
                    0x7F | 0x08 => {
                        // Backspace — delete char before cursor.
                        if cursor > 0 {
                            // Shift chars left.
                            for j in (cursor - 1)..line_len {
                                line[j] = line[j + 1];
                            }
                            line_len -= 1;
                            cursor -= 1;
                            // Redraw from cursor position to end.
                            // Move cursor back, write remaining, write space, move back.
                            syscalls::write(1, b"\x1B[D".as_ptr(), 3); // left
                            syscalls::write(1, line.as_ptr().add(cursor), line_len - cursor);
                            syscalls::write(1, b" ".as_ptr(), 1);
                            // Move cursor back to position.
                            let back = (line_len - cursor + 1) as u8;
                            let back_seq = [0x1B, b'[', b'0' + back / 10, b'0' + back % 10, b'D'];
                            // Simple: just send multiple \x1B[D
                            for _ in 0..(line_len - cursor + 1) {
                                syscalls::write(1, b"\x1B[D".as_ptr(), 3);
                            }
                        }
                        i += 1;
                        continue;
                    }
                    0x09 => {
                        // Tab — completion.
                        let result = features::tab_complete(&line[..line_len], cursor);
                        let new_line = result.line.as_slice();
                        let new_cursor = result.cursor;
                        // Clear current line and redraw.
                        clear_line(&line[..line_len], cursor);
                        let cn = new_line.len().min(LINE_MAX - 1);
                        for j in 0..cn {
                            line[j] = new_line[j];
                        }
                        line_len = cn;
                        cursor = new_cursor.min(cn);
                        if result.printed {
                            // We printed matches — reprint prompt + line.
                            syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());
                        }
                        syscalls::write(1, line.as_ptr(), line_len);
                        // Move cursor to the right position (we may have
                        // printed past it if printed=true). For simplicity,
                        // send cursor to end then back.
                        let back = line_len.saturating_sub(cursor);
                        for _ in 0..back {
                            syscalls::write(1, b"\x1B[D".as_ptr(), 3);
                        }
                        i += 1;
                        continue;
                    }
                    0x03 => {
                        // Ctrl-C — cancel line.
                        syscalls::write(1, b"^C\r\n".as_ptr(), 4);
                        line_len = 0;
                        cursor = 0;
                        break 'line_loop;
                    }
                    0x04 => {
                        // Ctrl-D — EOF on empty line.
                        if line_len == 0 {
                            syscalls::write(1, b"exit\r\n".as_ptr(), 6);
                            syscalls::exit(0);
                        }
                        i += 1;
                        continue;
                    }
                    c if c >= 0x20 && c < 0x7F => {
                        // Printable char — insert at cursor.
                        if line_len < LINE_MAX - 1 {
                            // Shift chars right.
                            for j in (cursor..line_len).rev() {
                                line[j + 1] = line[j];
                            }
                            line[cursor] = c;
                            line_len += 1;
                            // Echo the char + the rest of the line.
                            syscalls::write(1, line.as_ptr().add(cursor), line_len - cursor);
                            cursor += 1;
                            // Move cursor back to position.
                            let back = line_len - cursor;
                            for _ in 0..back {
                                syscalls::write(1, b"\x1B[D".as_ptr(), 3);
                            }
                        }
                        i += 1;
                        continue;
                    }
                    _ => {
                        // Other control char — ignore.
                        i += 1;
                        continue;
                    }
                }
            }
        }

        // Line submitted — null-terminate and process.
        line[line_len] = 0;
        if line_len == 0 {
            continue;
        }

        // Push to history BEFORE expansion (so !! works).
        features::history_push(&line[..line_len]);

        // History expansion.
        let expanded = features::history_expand(&line[..line_len]);
        // Tilde expansion (must come before variable expansion).
        let expanded = features::expand_tilde(expanded.as_slice());
        // Variable expansion ($VAR / ${VAR}).
        let expanded = features::expand_vars(expanded.as_slice());
        let expanded_slice = expanded.as_slice();

        // Check for pipe/redirect.
        let has_pipe_or_redirect = expanded_slice
            .iter()
            .any(|&b| b == b'|' || b == b'>' || b == b'<');
        if has_pipe_or_redirect {
            // Copy to static buffer for pipeline.
            static mut G_EXPANDED: [u8; LINE_MAX] = [0u8; LINE_MAX];
            let n = expanded_slice.len().min(LINE_MAX - 1);
            for j in 0..n {
                G_EXPANDED[j] = expanded_slice[j];
            }
            G_EXPANDED[n] = 0;
            let p = pipeline::parse(&G_EXPANDED[..n]);
            pipeline::execute(&G_EXPANDED[..n], &p);
            continue;
        }

        // Tokenize.
        let ntok = io::tokenize(expanded_slice, &mut G_TOKEN_OFFSETS);
        if ntok == 0 {
            continue;
        }

        // Glob expansion.
        static mut G_EXPANDED_ARGS: [[u8; 128]; 32] = [[0u8; 128]; 32];
        static mut G_ARGS: [&[u8]; 32] = [&[]; 32];
        let mut n_args = 0usize;
        for ti in 0..ntok {
            let (off, len) = G_TOKEN_OFFSETS[ti];
            let tok = &expanded_slice[off..off + len];
            let expansions = features::glob_expand(tok);
            for j in 0..expansions.len() {
                if n_args >= 32 {
                    break;
                }
                let exp = expansions[j];
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

        commands::dispatch(&G_ARGS[..n_args]);
    }
}

/// Clear the current line on screen: move cursor to start, overwrite with
/// spaces, move back to start. `line` is the current line content, `cursor`
/// is the cursor position within it.
unsafe fn clear_line(line: &[u8], _cursor: usize) {
    // Move cursor to start of line: send \r.
    syscalls::write(1, b"\r".as_ptr(), 1);
    // Overwrite the line with spaces.
    let len = line.len();
    for _ in 0..len {
        syscalls::write(1, b" ".as_ptr(), 1);
    }
    // Move back to start.
    syscalls::write(1, b"\r".as_ptr(), 1);
}

// ── Cooked-mode REPL (fallback) ─────────────────────────────────────────
//
// Used when the kernel doesn't support TIOCSRAW. Reads line-by-line via
// sys_read(0), which does echo + backspace + Enter internally. No tab
// completion or arrow-key history — but history expansion (!! / !N / !-N)
// and globbing still work because they operate on the submitted line.

unsafe fn cooked_mode_repl() -> ! {
    loop {
        syscalls::write(1, PROMPT.as_ptr(), PROMPT.len());
        let n = io::read_line(&mut G_LINE);
        if n == 0 {
            continue;
        }
        let mut end = n;
        while end > 0
            && (G_LINE[end - 1] == b'\n' || G_LINE[end - 1] == b'\r' || G_LINE[end - 1] == 0)
        {
            end -= 1;
        }
        if end == 0 {
            continue;
        }

        let raw = &G_LINE[..end];
        let expanded = features::history_expand(raw);
        let expanded = features::expand_tilde(expanded.as_slice());
        let expanded = features::expand_vars(expanded.as_slice());
        let expanded_slice = expanded.as_slice();
        features::history_push(expanded_slice);

        let has_pipe_or_redirect = expanded_slice
            .iter()
            .any(|&b| b == b'|' || b == b'>' || b == b'<');
        if has_pipe_or_redirect {
            static mut G_EXPANDED: [u8; LINE_MAX] = [0u8; LINE_MAX];
            let n = expanded_slice.len().min(LINE_MAX - 1);
            for j in 0..n {
                G_EXPANDED[j] = expanded_slice[j];
            }
            G_EXPANDED[n] = 0;
            let p = pipeline::parse(&G_EXPANDED[..n]);
            pipeline::execute(&G_EXPANDED[..n], &p);
            continue;
        }

        let ntok = io::tokenize(expanded_slice, &mut G_TOKEN_OFFSETS);
        if ntok == 0 {
            continue;
        }

        static mut G_EXPANDED_ARGS: [[u8; 128]; 32] = [[0u8; 128]; 32];
        static mut G_ARGS: [&[u8]; 32] = [&[]; 32];
        let mut n_args = 0usize;
        for ti in 0..ntok {
            let (off, len) = G_TOKEN_OFFSETS[ti];
            let tok = &expanded_slice[off..off + len];
            let expansions = features::glob_expand(tok);
            for j in 0..expansions.len() {
                if n_args >= 32 {
                    break;
                }
                let exp = expansions[j];
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

        commands::dispatch(&G_ARGS[..n_args]);
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
