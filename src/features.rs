//! Shell features: wildcard globbing + history persistence.
//!
//! Tab completion and arrow-key history navigation require raw terminal
//! mode (byte-at-a-time reads), which the kernel's UART line editor
//! doesn't currently expose. So this module implements:
//!
//!   1. Wildcard globbing (`*`, `?`, `[...]`) — works with the existing
//!      line-based read() because expansion happens AFTER the user
//!      presses Enter, on the already-tokenized line.
//!   2. Command history — stored in a ring buffer, but NOT navigable
//!      via arrow keys (no raw mode). Instead, a special `!!` token
//!      re-runs the most recent command, and `!N` re-runs command #N
//!      (1-indexed from the oldest available). This is the bash-style
//!      history expansion that works without raw mode.
//!
//! When the kernel grows a raw-mode syscall (SYS_fcntl F_GETFL/F_SETFL
//! with O_NONBLOCK, or a dedicated SYS_tcsetattr), tab completion and
//! arrow-key history can be layered on top.

use crate::io;
use crate::syscalls;

pub const HISTORY_SIZE: usize = 16;
pub const HISTORY_LINE_MAX: usize = 128;

/// Ring buffer of command history entries.
static mut G_HISTORY: [[u8; HISTORY_LINE_MAX]; HISTORY_SIZE] =
    [[0u8; HISTORY_LINE_MAX]; HISTORY_SIZE];
static mut G_HISTORY_LEN: [u8; HISTORY_SIZE] = [0u8; HISTORY_SIZE];
static mut G_HISTORY_COUNT: usize = 0; // total entries ever added (uncapped counter)

/// Add a line to the history. Trailing newlines are stripped. Empty
/// lines and lines identical to the most recent entry are skipped.
pub unsafe fn history_push(line: &[u8]) {
    // Strip trailing newline / CR / NUL.
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r' || line[end - 1] == 0) {
        end -= 1;
    }
    if end == 0 {
        return;
    }
    // Skip if identical to the most recent entry (bash behavior).
    let last_idx = if G_HISTORY_COUNT == 0 {
        usize::MAX
    } else {
        (G_HISTORY_COUNT - 1) % HISTORY_SIZE
    };
    if last_idx != usize::MAX {
        let prev_len = G_HISTORY_LEN[last_idx] as usize;
        let prev = &G_HISTORY[last_idx][..prev_len];
        if prev == &line[..end] {
            return;
        }
    }
    // Add.
    let slot = G_HISTORY_COUNT % HISTORY_SIZE;
    let n = end.min(HISTORY_LINE_MAX - 1);
    for i in 0..n {
        G_HISTORY[slot][i] = line[i];
    }
    G_HISTORY[slot][n] = 0;
    G_HISTORY_LEN[slot] = n as u8;
    G_HISTORY_COUNT = G_HISTORY_COUNT.saturating_add(1);
}

/// Get the Nth history entry (1-indexed from the oldest available).
/// Returns None if N is out of range.
pub unsafe fn history_get(n: usize) -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 || n == 0 {
        return None;
    }
    let stored = G_HISTORY_COUNT.min(HISTORY_SIZE);
    let oldest = G_HISTORY_COUNT.saturating_sub(stored);
    // User-visible index: oldest entry is #1, newest is #stored.
    let logical = oldest + (n - 1);
    if logical >= G_HISTORY_COUNT {
        return None;
    }
    let slot = logical % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

/// Get the most recent history entry (bash `!!`).
pub unsafe fn history_last() -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 {
        return None;
    }
    let slot = (G_HISTORY_COUNT - 1) % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

// ── History expansion ───────────────────────────────────────────────────

/// Expand history references in a line. Supports:
///   `!!`     → the previous command
///   `!N`     → command number N (1-indexed)
///   `!-N`    → the Nth command back from the current (1 = previous)
///
/// Returns the expanded line. If a reference can't be resolved, the
/// original token is left in place and an error message is printed.
pub unsafe fn history_expand(line: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'!' && i + 1 < line.len() {
            let next = line[i + 1];
            if next == b'!' {
                // !! → previous command
                if let Some(prev) = history_last() {
                    out.extend_from_slice(prev);
                } else {
                    io::write_error("no history yet");
                    out.push(b'!');
                    out.push(b'!');
                }
                i += 2;
                continue;
            }
            if next == b'-' {
                // !-N → Nth command back
                let mut j = i + 2;
                let mut n = 0usize;
                while j < line.len() && line[j] >= b'0' && line[j] <= b'9' {
                    n = n * 10 + (line[j] - b'0') as usize;
                    j += 1;
                }
                if n > 0 && j > i + 2 {
                    let stored = G_HISTORY_COUNT.min(HISTORY_SIZE);
                    if n <= stored {
                        let target = G_HISTORY_COUNT - n;
                        let slot = target % HISTORY_SIZE;
                        let len = G_HISTORY_LEN[slot] as usize;
                        out.extend_from_slice(&G_HISTORY[slot][..len]);
                    } else {
                        io::write_error("history index out of range");
                    }
                    i = j;
                    continue;
                }
            }
            if next >= b'0' && next <= b'9' {
                // !N → command number N
                let mut j = i + 1;
                let mut n = 0usize;
                while j < line.len() && line[j] >= b'0' && line[j] <= b'9' {
                    n = n * 10 + (line[j] - b'0') as usize;
                    j += 1;
                }
                if let Some(entry) = history_get(n) {
                    out.extend_from_slice(entry);
                } else {
                    io::write_error("history index out of range");
                }
                i = j;
                continue;
            }
        }
        out.push(line[i]);
        i += 1;
    }
    out
}

// ── Globbing ────────────────────────────────────────────────────────────

/// Check if a token contains glob characters (`*`, `?`, `[`).
pub fn has_glob(tok: &[u8]) -> bool {
    tok.iter().any(|&b| b == b'*' || b == b'?' || b == b'[')
}

/// Expand globs in a token. Returns a list of matching filesystem paths.
/// If no matches, returns the original token unchanged (POSIX behavior).
pub fn glob_expand(tok: &[u8]) -> Vec<[u8; 128]> {
    if !has_glob(tok) {
        let mut buf = [0u8; 128];
        let n = tok.len().min(127);
        buf[..n].copy_from_slice(&tok[..n]);
        let mut v = Vec::new();
        v.push(buf);
        return v;
    }
    let (dir_path, pattern) = split_dir_and_prefix(tok);
    let entries = scan_dir_entries(dir_path);
    let mut result: Vec<[u8; 128]> = Vec::new();
    for entry in entries.iter() {
        let entry_len = entry.iter().position(|&b| b == 0).unwrap_or(entry.len());
        let entry_name = &entry[..entry_len];
        if glob_match(pattern, entry_name) {
            // Build the full path: dir_path + entry_name.
            let mut buf = [0u8; 128];
            let mut pos = 0;
            let dn = dir_path.len().min(64);
            for i in 0..dn {
                buf[pos] = dir_path[i];
                pos += 1;
            }
            let fn_len = entry_name.len().min(127 - pos);
            for i in 0..fn_len {
                buf[pos] = entry_name[i];
                pos += 1;
            }
            result.push(buf);
        }
    }
    if result.is_empty() {
        // No matches — return the original token unchanged.
        let mut buf = [0u8; 128];
        let n = tok.len().min(127);
        buf[..n].copy_from_slice(&tok[..n]);
        let mut v = Vec::new();
        v.push(buf);
        v
    } else {
        result
    }
}

/// Split a token into (directory_path, file_prefix).
/// `/tmp/foo*` → (`/tmp/`, `foo*`)
/// `foo*`      → (cwd, `foo*`)
/// `/`         → (`/`, ``)
fn split_dir_and_prefix(tok: &[u8]) -> (&[u8], &[u8]) {
    match tok.iter().rposition(|&b| b == b'/') {
        Some(idx) => (&tok[..idx + 1], &tok[idx + 1..]),
        None => (b"", tok),
    }
}

/// Scan a directory and return a list of entry names.
/// Uses the kernel's readdir() syscall (stateful, path-based).
fn scan_dir_entries(dir_path: &[u8]) -> Vec<[u8; 64]> {
    let mut result: Vec<[u8; 64]> = Vec::new();
    // Build a NUL-terminated path for the directory.
    let mut path_buf = [0u8; 256];
    let n = dir_path.len().min(255);
    for i in 0..n {
        path_buf[i] = dir_path[i];
    }
    path_buf[n] = 0;
    // If dir_path is empty, use "/".
    if n == 0 {
        path_buf[0] = b'/';
        path_buf[1] = 0;
    }
    // readdir returns one entry per call via a stateful cursor in the kernel.
    let mut name_buf = [0u8; 256];
    for _ in 0..64 {
        let ret = unsafe { syscalls::readdir(path_buf.as_ptr(), name_buf.as_mut_ptr(), 255) };
        if ret <= 0 {
            break;
        }
        let mut elen = 0;
        while elen < 255 && name_buf[elen] != 0 {
            elen += 1;
        }
        let mut entry = [0u8; 64];
        let cn = elen.min(63);
        entry[..cn].copy_from_slice(&name_buf[..cn]);
        result.push(entry);
    }
    result
}

/// Match a glob pattern against a filename. Supports `*` (any sequence),
/// `?` (any single char), and `[...]` (character class with ranges).
pub fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    glob_match_helper(pattern, 0, name, 0)
}

fn glob_match_helper(pattern: &[u8], pi: usize, name: &[u8], ni: usize) -> bool {
    let mut pi = pi;
    let mut ni = ni;
    while pi < pattern.len() {
        match pattern[pi] {
            b'*' => {
                pi += 1;
                if pi >= pattern.len() {
                    return true;
                }
                while ni <= name.len() {
                    if glob_match_helper(pattern, pi, name, ni) {
                        return true;
                    }
                    ni += 1;
                }
                return false;
            }
            b'?' => {
                if ni >= name.len() {
                    return false;
                }
                ni += 1;
                pi += 1;
            }
            b'[' => {
                if ni >= name.len() {
                    return false;
                }
                let mut end = pi + 1;
                while end < pattern.len() && pattern[end] != b']' {
                    end += 1;
                }
                if end >= pattern.len() {
                    if name[ni] != b'[' {
                        return false;
                    }
                    ni += 1;
                    pi += 1;
                    continue;
                }
                let class = &pattern[pi + 1..end];
                let c = name[ni];
                let mut matched = false;
                let mut i = 0;
                while i < class.len() {
                    if i + 2 < class.len() && class[i + 1] == b'-' {
                        if c >= class[i] && c <= class[i + 2] {
                            matched = true;
                            break;
                        }
                        i += 3;
                    } else {
                        if c == class[i] {
                            matched = true;
                            break;
                        }
                        i += 1;
                    }
                }
                if !matched {
                    return false;
                }
                ni += 1;
                pi = end + 1;
            }
            c => {
                if ni >= name.len() || name[ni] != c {
                    return false;
                }
                ni += 1;
                pi += 1;
            }
        }
    }
    ni == name.len()
}

// ── Minimal Vec implementation (no heap, static backing) ────────────────
//
// We can't use alloc::vec::Vec in no_std without the alloc feature, and
// OnyxShell is built with no_std + no_main. So we use a simple fixed-capacity
// Vec backed by a static array. We use a const default rather than the
// Default trait because arrays > 32 elements don't implement Default in
// stable Rust.

pub struct Vec<T: Copy> {
    items: [T; 256],
    len: usize,
}

impl<T: Copy + ConstDefault> Vec<T> {
    pub fn new() -> Self {
        Vec {
            items: [T::CONST_DEFAULT; 256],
            len: 0,
        }
    }
    pub fn push(&mut self, item: T) {
        if self.len < 256 {
            self.items[self.len] = item;
            self.len += 1;
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn iter(&self) -> core::slice::Iter<T> {
        self.items[..self.len].iter()
    }
    pub fn extend_from_slice(&mut self, other: &[T]) {
        for &item in other {
            self.push(item);
        }
    }
}

impl<T: Copy + ConstDefault> core::ops::Index<usize> for Vec<T> {
    type Output = T;
    fn index(&self, i: usize) -> &T {
        &self.items[i]
    }
}

/// Trait like Default but with a const fn, so we can use it in array
/// initialization for arrays of any size.
pub trait ConstDefault: Copy {
    const CONST_DEFAULT: Self;
}

impl ConstDefault for u8 {
    const CONST_DEFAULT: Self = 0;
}

impl ConstDefault for [u8; 64] {
    const CONST_DEFAULT: Self = [0u8; 64];
}

impl ConstDefault for [u8; 128] {
    const CONST_DEFAULT: Self = [0u8; 128];
}

impl Vec<u8> {
    pub fn as_slice(&self) -> &[u8] {
        &self.items[..self.len]
    }
}
