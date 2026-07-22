//! Command history ring buffer with navigation and expansion.

use crate::features::buffer::Vec;
use crate::io;

pub const HISTORY_SIZE: usize = 16;
pub const HISTORY_LINE_MAX: usize = 128;

static mut G_HISTORY: [[u8; HISTORY_LINE_MAX]; HISTORY_SIZE] =
    [[0u8; HISTORY_LINE_MAX]; HISTORY_SIZE];
static mut G_HISTORY_LEN: [u8; HISTORY_SIZE] = [0u8; HISTORY_SIZE];
static mut G_HISTORY_COUNT: usize = 0;

pub unsafe fn history_push(line: &[u8]) {
    let mut end = line.len();
    while end > 0 && (line[end - 1] == b'\n' || line[end - 1] == b'\r' || line[end - 1] == 0) {
        end -= 1;
    }
    if end == 0 {
        return;
    }
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
    let slot = G_HISTORY_COUNT % HISTORY_SIZE;
    let n = end.min(HISTORY_LINE_MAX - 1);
    for i in 0..n {
        G_HISTORY[slot][i] = line[i];
    }
    G_HISTORY[slot][n] = 0;
    G_HISTORY_LEN[slot] = n as u8;
    G_HISTORY_COUNT = G_HISTORY_COUNT.saturating_add(1);
}

pub unsafe fn history_get(n: usize) -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 || n == 0 {
        return None;
    }
    let stored = G_HISTORY_COUNT.min(HISTORY_SIZE);
    let oldest = G_HISTORY_COUNT.saturating_sub(stored);
    let logical = oldest + (n - 1);
    if logical >= G_HISTORY_COUNT {
        return None;
    }
    let slot = logical % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

pub unsafe fn history_last() -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 {
        return None;
    }
    let slot = (G_HISTORY_COUNT - 1) % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

// ── Arrow-key history navigation ───────────────────────────────────────

static mut G_NAV_CURSOR: isize = -1;

pub unsafe fn nav_reset() {
    G_NAV_CURSOR = -1;
}

pub unsafe fn nav_up() -> Option<&'static [u8]> {
    if G_HISTORY_COUNT == 0 {
        return None;
    }
    let stored = G_HISTORY_COUNT.min(HISTORY_SIZE) as isize;
    let new_cursor = if G_NAV_CURSOR == -1 {
        0
    } else {
        G_NAV_CURSOR + 1
    };
    if new_cursor >= stored {
        return None;
    }
    G_NAV_CURSOR = new_cursor;
    let logical = G_HISTORY_COUNT - 1 - new_cursor as usize;
    let slot = logical % HISTORY_SIZE;
    let len = G_HISTORY_LEN[slot] as usize;
    Some(&G_HISTORY[slot][..len])
}

pub unsafe fn nav_down() -> Option<&'static [u8]> {
    if G_NAV_CURSOR <= 0 {
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

pub unsafe fn history_expand(line: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'!' && i + 1 < line.len() {
            let next = line[i + 1];
            if next == b'!' {
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
                let mut j = i + 2;
                let mut n = 0usize;
                while j < line.len() && line[j] >= b'0' && line[j] <= b'9' {
                    n = match n
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((line[j] - b'0') as usize))
                    {
                        Some(v) => v,
                        None => break,
                    };
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
                let mut j = i + 1;
                let mut n = 0usize;
                while j < line.len() && line[j] >= b'0' && line[j] <= b'9' {
                    n = match n
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((line[j] - b'0') as usize))
                    {
                        Some(v) => v,
                        None => break,
                    };
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
