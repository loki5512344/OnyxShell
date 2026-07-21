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

// ── Arrow-key history navigation ───────────────────────────────────────
//
// The shell maintains a "cursor" into the history. When the user presses
// Up, the cursor moves to the previous entry; Down moves to the next.
// The cursor is separate from G_HISTORY_COUNT (which tracks total entries)
// so that navigating doesn't perturb the push order.

static mut G_NAV_CURSOR: isize = -1; // -1 = "current line", 0..N = history index from newest

/// Reset the navigation cursor (call when a new line starts).
pub unsafe fn nav_reset() {
    G_NAV_CURSOR = -1;
}

/// Navigate history up (older). Returns the entry to display, or None
/// if we're already at the oldest entry.
pub unsafe fn nav_up() -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 {
        return None;
    }
    let stored = G_HISTORY_COUNT.min(HISTORY_SIZE) as isize;
    // G_NAV_CURSOR == -1 means "current line" (not in history).
    // 0 means newest entry, stored-1 means oldest.
    let new_cursor = if G_NAV_CURSOR == -1 {
        0 // start at newest
    } else {
        G_NAV_CURSOR + 1
    };
    if new_cursor >= stored {
        return None; // already at oldest
    }
    G_NAV_CURSOR = new_cursor;
    // Convert cursor (0=newest) to slot index.
    let logical = G_HISTORY_COUNT - 1 - new_cursor as usize;
    let slot = logical % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

/// Navigate history down (newer). Returns the entry to display, or
/// None if we're back at the "current line" (cursor == -1).
pub unsafe fn nav_down() -> Option<&'static [u8]> {
    if G_NAV_CURSOR <= 0 {
        // Back to current line.
        G_NAV_CURSOR = -1;
        return None;
    }
    G_NAV_CURSOR -= 1;
    let logical = G_HISTORY_COUNT - 1 - G_NAV_CURSOR as usize;
    let slot = logical % HISTORY_SIZE;
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

fn entry_startswith(entry: &[u8], prefix: &[u8]) -> bool {
    if entry.len() < prefix.len() {
        return false;
    }
    &entry[..prefix.len()] == prefix
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

impl ConstDefault for usize {
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

// ── Environment variables ──────────────────────────────────────────────
//
// Simple flat array of (key, value) pairs. Max 32 entries, key ≤ 64 bytes,
// value ≤ 128 bytes. Lookup is O(N) — fine for interactive use.

pub const ENV_MAX: usize = 32;
pub const ENV_KEY_MAX: usize = 64;
pub const ENV_VAL_MAX: usize = 128;

static mut G_ENV_KEYS: [[u8; ENV_KEY_MAX]; ENV_MAX] = [[0; ENV_KEY_MAX]; ENV_MAX];
static mut G_ENV_KEY_LEN: [u8; ENV_MAX] = [0; ENV_MAX];
static mut G_ENV_VALS: [[u8; ENV_VAL_MAX]; ENV_MAX] = [[0; ENV_VAL_MAX]; ENV_MAX];
static mut G_ENV_VAL_LEN: [u8; ENV_MAX] = [0; ENV_MAX];
static mut G_ENV_COUNT: usize = 0;

/// Initialise default environment (HOME and PATH).
pub unsafe fn env_init() {
    // These are the fallback values before the init process sets them.
    env_set(b"HOME", b"/users/root");
    env_set(b"PATH", b"/bin");
}

/// Look up a variable by name. Returns `None` if not found.
pub unsafe fn env_get(key: &[u8]) -> Option<&'static [u8]> {
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        if &G_ENV_KEYS[i][..klen] == key {
            let vlen = G_ENV_VAL_LEN[i] as usize;
            return Some(&G_ENV_VALS[i][..vlen]);
        }
    }
    None
}

/// Set (or update) a variable. If the key is empty or storage is full
/// the call is silently ignored.
pub unsafe fn env_set(key: &[u8], val: &[u8]) {
    if key.is_empty() {
        return;
    }
    let kn = key.len().min(ENV_KEY_MAX - 1);
    let vn = val.len().min(ENV_VAL_MAX - 1);

    // Update existing entry.
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        if &G_ENV_KEYS[i][..klen] == key {
            for j in 0..vn {
                G_ENV_VALS[i][j] = val[j];
            }
            G_ENV_VALS[i][vn] = 0;
            G_ENV_VAL_LEN[i] = vn as u8;
            return;
        }
    }

    // Add new entry.
    if G_ENV_COUNT >= ENV_MAX {
        return;
    }
    let slot = G_ENV_COUNT;
    for j in 0..kn {
        G_ENV_KEYS[slot][j] = key[j];
    }
    G_ENV_KEYS[slot][kn] = 0;
    G_ENV_KEY_LEN[slot] = kn as u8;
    for j in 0..vn {
        G_ENV_VALS[slot][j] = val[j];
    }
    G_ENV_VALS[slot][vn] = 0;
    G_ENV_VAL_LEN[slot] = vn as u8;
    G_ENV_COUNT += 1;
}

/// Remove a variable. No-op if the key does not exist.
pub unsafe fn env_unset(key: &[u8]) {
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        if &G_ENV_KEYS[i][..klen] == key {
            // Shift remaining entries left to fill the hole.
            for j in i..G_ENV_COUNT - 1 {
                G_ENV_KEYS[j] = G_ENV_KEYS[j + 1];
                G_ENV_KEY_LEN[j] = G_ENV_KEY_LEN[j + 1];
                G_ENV_VALS[j] = G_ENV_VALS[j + 1];
                G_ENV_VAL_LEN[j] = G_ENV_VAL_LEN[j + 1];
            }
            G_ENV_COUNT -= 1;
            return;
        }
    }
}

/// Print every stored variable as `KEY=VALUE`, one per line.
pub unsafe fn env_list() {
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        let vlen = G_ENV_VAL_LEN[i] as usize;
        io::write_raw(&G_ENV_KEYS[i][..klen]);
        io::write_raw(b"=");
        io::write_raw(&G_ENV_VALS[i][..vlen]);
        io::newline();
    }
}

/// Build a NULL-terminated `envp` array for `execve`. Each entry is a
/// NUL-terminated `KEY=VALUE` string.  Returns the number of entries
/// written (the pointer array is terminated by an extra NULL).
pub unsafe fn build_envp(
    strings: &mut [[u8; ENV_KEY_MAX + ENV_VAL_MAX + 2]; ENV_MAX],
    ptrs: &mut [u64; ENV_MAX + 1],
) -> usize {
    let mut count = 0;
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        let vlen = G_ENV_VAL_LEN[i] as usize;
        let mut pos = 0;
        for j in 0..klen {
            strings[count][pos] = G_ENV_KEYS[i][j];
            pos += 1;
        }
        strings[count][pos] = b'=';
        pos += 1;
        for j in 0..vlen {
            strings[count][pos] = G_ENV_VALS[i][j];
            pos += 1;
        }
        strings[count][pos] = 0;
        ptrs[count] = strings[count].as_ptr() as u64;
        count += 1;
    }
    ptrs[count] = 0;
    count
}

// ── Variable expansion (`$VAR` / `${VAR}`) ────────────────────────────

/// Expand `$VAR` and `${VAR}` references inside a line.
/// Undefined variables expand to the empty string (POSIX behaviour).
pub unsafe fn expand_vars(line: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'$' && i + 1 < line.len() {
            let next = line[i + 1];
            if next == b'{' {
                // ${VAR} — read until closing '}'.
                let mut j = i + 2;
                while j < line.len() && line[j] != b'}' {
                    j += 1;
                }
                if j < line.len() {
                    let varname = &line[i + 2..j];
                    if let Some(val) = env_get(varname) {
                        out.extend_from_slice(val);
                    }
                    i = j + 1;
                    continue;
                }
            } else if (next >= b'A' && next <= b'Z')
                || (next >= b'a' && next <= b'z')
                || next == b'_'
            {
                // $VAR — consume alphanumeric / underscore characters.
                let mut j = i + 1;
                while j < line.len()
                    && ((line[j] >= b'A' && line[j] <= b'Z')
                        || (line[j] >= b'a' && line[j] <= b'z')
                        || (line[j] >= b'0' && line[j] <= b'9')
                        || line[j] == b'_')
                {
                    j += 1;
                }
                let varname = &line[i + 1..j];
                if let Some(val) = env_get(varname) {
                    out.extend_from_slice(val);
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

// ── Tilde expansion (`~` / `~user`) ──────────────────────────────────

/// Expand `~` → value of `$HOME` (or `/users/root`), and
/// `~user` → `/users/user/`.  Tilde is only recognised when it appears
/// at the start of a word (i.e.  beginning of line or after whitespace).
pub unsafe fn expand_tilde(line: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'~' && (i == 0 || line[i - 1] == b' ' || line[i - 1] == b'\t') {
            let mut j = i + 1;
            while j < line.len() && line[j] != b'/' && !io::is_whitespace(line[j]) {
                j += 1;
            }
            if j == i + 1 {
                // "~" alone — use HOME.
                if let Some(home) = env_get(b"HOME") {
                    out.extend_from_slice(home);
                } else {
                    out.extend_from_slice(b"/users/root");
                }
            } else {
                // "~username" — assume /users/<name>.
                out.extend_from_slice(b"/users/");
                out.extend_from_slice(&line[i + 1..j]);
            }
            i = j;
            continue;
        }
        out.push(line[i]);
        i += 1;
    }
    out
}

// ── Tab completion ─────────────────────────────────────────────────────
//
// When the user presses Tab, the shell calls tab_complete() with the
// current line buffer and cursor position. The function:
//   1. Finds the token under the cursor.
//   2. If it's the first word, matches against built-in command names.
//   3. Always matches against filesystem entries in the token's directory.
//   4. If one match: fills in the full name + trailing space.
//   5. If multiple matches: fills in the common prefix; if no new prefix,
//      prints all matches.
//   6. Returns the new line contents and new cursor position.
//
// When the user presses Tab, the shell calls tab_complete() with the
// current line buffer and cursor position. The function:
//   1. Finds the token under the cursor.
//   2. If it's the first word, matches against built-in command names.
//   3. Always matches against filesystem entries in the token's directory.
//   4. If one match: fills in the full name + trailing space.
//   5. If multiple matches: fills in the common prefix; if no new prefix,
//      prints all matches.
//   6. Returns the new line contents and new cursor position.

/// Built-in command names for tab completion of the first word.
const BUILTINS: &[&[u8]] = &[
    b"ls", b"cat", b"cp", b"mv", b"rm", b"mkdir", b"touch", b"stat", b"cd", b"pwd", b"echo",
    b"whoami", b"uname", b"date", b"clear", b"help", b"exit", b"exec", b"run", b"ver", b"export",
    b"set", b"unset", b"jobs", b"fg", b"bg",
];
/// Result of a tab completion attempt.
pub struct TabResult {
    /// New line contents (may be unchanged if no matches).
    pub line: Vec<u8>,
    /// New cursor position.
    pub cursor: usize,
    /// True if we printed matches to the screen (so the caller should
    /// reprint the prompt + line on a fresh line).
    pub printed: bool,
}

/// Attempt tab completion. `line` is the current input, `cursor` is the
/// byte offset of the cursor within `line`.
pub unsafe fn tab_complete(line: &[u8], cursor: usize) -> TabResult {
    // Find the start of the current token (last whitespace before cursor).
    let mut tok_start = cursor;
    while tok_start > 0 && !io::is_whitespace(line[tok_start - 1]) {
        tok_start -= 1;
    }
    let tok = &line[tok_start..cursor];
    let is_first_word = {
        let mut i = 0;
        while i < tok_start && io::is_whitespace(line[i]) {
            i += 1;
        }
        i == tok_start
    };

    // Collect matches.
    let mut matches: Vec<[u8; 64]> = Vec::new();
    let mut match_lens: Vec<usize> = Vec::new();

    if is_first_word {
        for &builtin in BUILTINS {
            if builtin.starts_with(tok) {
                let mut buf = [0u8; 64];
                let n = builtin.len().min(63);
                buf[..n].copy_from_slice(&builtin[..n]);
                matches.push(buf);
                match_lens.push(n);
            }
        }
    }

    // Filesystem completion.
    let (dir_path, file_prefix) = split_dir_and_prefix(tok);
    let entries = scan_dir_entries(dir_path);
    for entry in entries.iter() {
        let entry_len = entry.iter().position(|&b| b == 0).unwrap_or(entry.len());
        if entry_startswith(&entry[..entry_len], file_prefix) {
            let mut buf = [0u8; 64];
            let n = entry_len.min(63);
            buf[..n].copy_from_slice(&entry[..n]);
            matches.push(buf);
            match_lens.push(n);
        }
    }

    if matches.is_empty() {
        return TabResult {
            line: line_to_vec(line),
            cursor,
            printed: false,
        };
    }

    if matches.len() == 1 {
        // Single match — fill in the full entry + a trailing space.
        let m = &matches[0];
        let m_len = match_lens[0];
        let mut new_line: Vec<u8> = Vec::new();
        new_line.extend_from_slice(&line[..tok_start]);
        new_line.extend_from_slice(&m[..m_len]);
        new_line.push(b' ');
        new_line.extend_from_slice(&line[cursor..]);
        let new_cursor = tok_start + m_len + 1;
        return TabResult {
            line: new_line,
            cursor: new_cursor,
            printed: false,
        };
    }

    // Multiple matches — fill in the common prefix.
    let mut common = match_lens[0];
    for i in 1..matches.len() {
        let m = &matches[i];
        let m_len = match_lens[i];
        let cmp_len = common.min(m_len);
        let mut j = 0;
        while j < cmp_len && matches[0][j] == m[j] {
            j += 1;
        }
        common = j;
        if common == 0 {
            break;
        }
    }
    let mut printed = false;
    if common > tok.len() {
        // We can extend the token with the common prefix.
        let mut new_line: Vec<u8> = Vec::new();
        new_line.extend_from_slice(&line[..tok_start]);
        new_line.extend_from_slice(&matches[0][..common]);
        new_line.extend_from_slice(&line[cursor..]);
        let new_cursor = tok_start + common;
        // Also print all matches so the user sees the options.
        io::newline();
        for i in 0..matches.len() {
            let m_len = match_lens[i];
            io::write_raw(&matches[i][..m_len]);
            io::write_raw(b"  ");
        }
        io::newline();
        printed = true;
        TabResult {
            line: new_line,
            cursor: new_cursor,
            printed,
        }
    } else {
        // No common prefix to add — just print the matches.
        io::newline();
        for i in 0..matches.len() {
            let m_len = match_lens[i];
            io::write_raw(&matches[i][..m_len]);
            io::write_raw(b"  ");
        }
        io::newline();
        printed = true;
        TabResult {
            line: line_to_vec(line),
            cursor,
            printed,
        }
    }
}

// ── Background job management ──────────────────────────────────────────
//
// Simple flat array of background jobs (PID + running flag).
// Jobs are numbered sequentially with a global counter.

pub const JOB_MAX: usize = 16;

static mut G_JOB_IDS: [usize; JOB_MAX] = [0; JOB_MAX];
static mut G_JOB_PIDS: [i32; JOB_MAX] = [0; JOB_MAX];
static mut G_JOB_RUNNING: [bool; JOB_MAX] = [false; JOB_MAX];
static mut G_JOB_NEXT_ID: usize = 1;
static mut G_JOB_COUNT: usize = 0;

/// Add a background job. Returns the job number (for [N] display), or 0 if full.
pub unsafe fn job_add(pid: i32) -> usize {
    if G_JOB_COUNT >= JOB_MAX {
        return 0;
    }
    let idx = G_JOB_COUNT;
    let job_id = G_JOB_NEXT_ID;
    G_JOB_NEXT_ID = G_JOB_NEXT_ID.wrapping_add(1);
    G_JOB_IDS[idx] = job_id;
    G_JOB_PIDS[idx] = pid;
    G_JOB_RUNNING[idx] = true;
    G_JOB_COUNT += 1;
    job_id
}

/// Remove a job by its job number. Returns true if found.
pub unsafe fn job_remove_by_id(job_id: usize) -> bool {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            for j in i..G_JOB_COUNT - 1 {
                G_JOB_IDS[j] = G_JOB_IDS[j + 1];
                G_JOB_PIDS[j] = G_JOB_PIDS[j + 1];
                G_JOB_RUNNING[j] = G_JOB_RUNNING[j + 1];
            }
            G_JOB_COUNT -= 1;
            return true;
        }
    }
    false
}

/// Find a job by number. Returns (id, pid, running) if found.
pub unsafe fn job_find_by_id(job_id: usize) -> Option<(usize, i32, bool)> {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            return Some((G_JOB_IDS[i], G_JOB_PIDS[i], G_JOB_RUNNING[i]));
        }
    }
    None
}

/// Mark a job as running (or not).
pub unsafe fn job_set_running(job_id: usize, running: bool) -> bool {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            G_JOB_RUNNING[i] = running;
            return true;
        }
    }
    false
}

/// Number of tracked background jobs.
#[allow(dead_code)]
pub unsafe fn job_count() -> usize {
    G_JOB_COUNT
}

/// Get job info by index in the internal array (0..job_count).
#[allow(dead_code)]
pub unsafe fn job_get_by_index(idx: usize) -> Option<(usize, i32, bool)> {
    if idx >= G_JOB_COUNT {
        return None;
    }
    Some((G_JOB_IDS[idx], G_JOB_PIDS[idx], G_JOB_RUNNING[idx]))
}

/// Print the job list (for the `jobs` built-in).
pub unsafe fn job_list() {
    for i in 0..G_JOB_COUNT {
        io::write_raw(b"[");
        io::write_u64(G_JOB_IDS[i] as u64);
        io::write_raw(b"] ");
        io::write_i64(G_JOB_PIDS[i] as i64);
        io::write_raw(b" ");
        if G_JOB_RUNNING[i] {
            io::write_raw(b"Running");
        } else {
            io::write_raw(b"Done");
        }
        io::newline();
    }
}

/// Poll for completed children with waitpid(WNOHANG).
/// Removes finished jobs and prints "[N] PID Done".
pub unsafe fn job_reap() {
    loop {
        let mut status: i32 = 0;
        let pid = crate::syscalls::waitpid(0xFFFF_FFFF, &mut status, crate::syscalls::WNOHANG);
        if pid <= 0 {
            break;
        }
        let mut found = false;
        for i in 0..G_JOB_COUNT {
            if G_JOB_PIDS[i] as i64 == pid {
                io::write_raw(b"[");
                io::write_u64(G_JOB_IDS[i] as u64);
                io::write_raw(b"] ");
                io::write_i64(pid);
                io::write_raw(b" Done\n");
                for j in i..G_JOB_COUNT - 1 {
                    G_JOB_IDS[j] = G_JOB_IDS[j + 1];
                    G_JOB_PIDS[j] = G_JOB_PIDS[j + 1];
                    G_JOB_RUNNING[j] = G_JOB_RUNNING[j + 1];
                }
                G_JOB_COUNT -= 1;
                found = true;
                break;
            }
        }
        if !found {
            // Child wasn't one of our tracked jobs — just ignore.
        }
    }
}

fn line_to_vec(line: &[u8]) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(line);
    v
}
