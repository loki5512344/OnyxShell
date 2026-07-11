//! I/O helpers — thin wrappers over syscalls that make printing and
//! reading easier from the shell's command implementations.

use crate::syscalls;

/// Write a raw byte slice to stdout (fd 1).
#[inline]
pub fn write_raw(bytes: &[u8]) {
    unsafe {
        syscalls::write(1, bytes.as_ptr(), bytes.len());
    }
}

/// Write a string slice to stdout.
#[inline]
pub fn write_str(s: &str) {
    write_raw(s.as_bytes());
}

/// Write a single byte to stdout.
#[inline]
pub fn write_byte(b: u8) {
    unsafe {
        let buf: [u8; 2] = [b, 0];
        syscalls::write(1, buf.as_ptr(), 1);
    }
}

/// Write a newline (`\n` — the kernel's UART driver expands it to `\r\n`).
#[inline]
pub fn newline() {
    write_raw(b"\n");
}

/// Write `s` followed by a newline.
#[inline]
pub fn write_line(s: &str) {
    write_str(s);
    newline();
}

/// Write an error message: `osh: <msg>\n`.
pub fn write_error(msg: &str) {
    write_str("osh: ");
    write_str(msg);
    newline();
}

/// Write an error with errno context: `osh: <msg>: <errno_str>\n`.
pub fn write_error_errno(msg: &str, ret: i64) {
    write_str("osh: ");
    write_str(msg);
    write_str(": ");
    write_raw(syscalls::errno_str(ret));
    newline();
}

/// Print an unsigned 64-bit number in decimal.
pub fn write_u64(n: u64) {
    if n == 0 {
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = n;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    write_raw(&buf[i..]);
}

/// Print a signed 64-bit number in decimal.
pub fn write_i64(n: i64) {
    if n < 0 {
        write_byte(b'-');
        write_u64(n.unsigned_abs());
    } else {
        write_u64(n as u64);
    }
}

/// Print an unsigned number in hexadecimal with `0x` prefix.
pub fn write_hex(n: u64) {
    write_raw(b"0x");
    if n == 0 {
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 16];
    let mut i = buf.len();
    let mut n = n;
    const HEX: &[u8] = b"0123456789abcdef";
    while n > 0 {
        i -= 1;
        buf[i] = HEX[(n & 0xf) as usize];
        n >>= 4;
    }
    write_raw(&buf[i..]);
}

/// Print a number in a fixed-width field (right-aligned with spaces).
pub fn write_u64_field(n: u64, width: usize) {
    if n == 0 {
        for _ in 0..(width - 1) { write_byte(b' '); }
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    let mut n = n;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    let digits = buf.len() - i;
    if digits < width {
        for _ in 0..(width - digits) { write_byte(b' '); }
    }
    write_raw(&buf[i..]);
}

/// Read a line from stdin (fd 0).
///
/// The kernel's `sys_read` for fd 0 performs line editing:
/// echo, backspace (0x7F/0x08), and Enter to submit. The returned
/// buffer is NUL-terminated and includes the trailing `\n`.
///
/// Returns the number of bytes read (including the trailing `\n`, excluding the NUL).
/// Returns 0 if the read fails or the line is empty (just Enter).
pub fn read_line(buf: &mut [u8]) -> usize {
    let n = unsafe { syscalls::read(0, buf.as_mut_ptr(), buf.len() as u64) };
    if n <= 0 {
        return 0;
    }
    n as usize
}

/// Check whether a character is whitespace (space, tab, \r, \n).
#[inline]
pub fn is_whitespace(b: u8) -> bool {
    b == b' ' || b == b'\t' || b == b'\r' || b == b'\n'
}

/// Split a byte slice into trimmed tokens (by whitespace).
/// Returns a slice of (start, len) tuples into the original buffer.
pub fn tokenize<'a>(line: &'a [u8], tokens: &mut [(usize, usize); 16]) -> usize {
    let mut count = 0;
    let mut i = 0;
    let len = line.len();
    while i < len && count < tokens.len() {
        // Skip whitespace.
        while i < len && is_whitespace(line[i]) {
            i += 1;
        }
        if i >= len {
            break;
        }
        let start = i;
        // Read token.
        while i < len && !is_whitespace(line[i]) {
            i += 1;
        }
        tokens[count] = (start, i - start);
        count += 1;
    }
    count
}
