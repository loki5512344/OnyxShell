//! Environment variable storage and variable/tilde expansion.

use crate::features::buffer::Vec;
use crate::io;

pub const ENV_MAX: usize = 32;
pub const ENV_KEY_MAX: usize = 64;
pub const ENV_VAL_MAX: usize = 128;

static mut G_ENV_KEYS: [[u8; ENV_KEY_MAX]; ENV_MAX] = [[0; ENV_KEY_MAX]; ENV_MAX];
static mut G_ENV_KEY_LEN: [u8; ENV_MAX] = [0; ENV_MAX];
static mut G_ENV_VALS: [[u8; ENV_VAL_MAX]; ENV_MAX] = [[0; ENV_VAL_MAX]; ENV_MAX];
static mut G_ENV_VAL_LEN: [u8; ENV_MAX] = [0; ENV_MAX];
static mut G_ENV_COUNT: usize = 0;

pub unsafe fn env_init() {
    env_set(b"HOME", b"/users/root");
    env_set(b"PATH", b"/bin");
}

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

pub unsafe fn env_set(key: &[u8], val: &[u8]) {
    if key.is_empty() {
        return;
    }
    let kn = key.len().min(ENV_KEY_MAX - 1);
    let vn = val.len().min(ENV_VAL_MAX - 1);

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

pub unsafe fn env_unset(key: &[u8]) {
    for i in 0..G_ENV_COUNT {
        let klen = G_ENV_KEY_LEN[i] as usize;
        if &G_ENV_KEYS[i][..klen] == key {
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

// ── Variable expansion ────────────────────────────────────────────────

pub unsafe fn expand_vars(line: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < line.len() {
        if line[i] == b'$' && i + 1 < line.len() {
            let next = line[i + 1];
            if next == b'{' {
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

// ── Tilde expansion ──────────────────────────────────────────────────

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
                if let Some(home) = env_get(b"HOME") {
                    out.extend_from_slice(home);
                } else {
                    out.extend_from_slice(b"/users/root");
                }
            } else {
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
