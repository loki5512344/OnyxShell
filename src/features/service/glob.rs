//! Wildcard glob matching (`*`, `?`, `[...]`) with iterative
//! two-pointer algorithm (O(1) stack, no recursion).

use crate::features::buffer::Vec;
use crate::syscalls;

pub fn has_glob(tok: &[u8]) -> bool {
    tok.iter().any(|&b| b == b'*' || b == b'?' || b == b'[')
}

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

pub fn split_dir_and_prefix(tok: &[u8]) -> (&[u8], &[u8]) {
    match tok.iter().rposition(|&b| b == b'/') {
        Some(idx) => (&tok[..idx + 1], &tok[idx + 1..]),
        None => (b"", tok),
    }
}

pub fn entry_startswith(entry: &[u8], prefix: &[u8]) -> bool {
    if entry.len() < prefix.len() {
        return false;
    }
    &entry[..prefix.len()] == prefix
}

pub fn scan_dir_entries(dir_path: &[u8]) -> Vec<[u8; 64]> {
    let mut result: Vec<[u8; 64]> = Vec::new();
    let mut path_buf = [0u8; 256];
    let n = dir_path.len().min(255);
    for i in 0..n {
        path_buf[i] = dir_path[i];
    }
    path_buf[n] = 0;
    if n == 0 {
        path_buf[0] = b'/';
        path_buf[1] = 0;
    }
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

pub fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    let mut pi = 0usize;
    let mut ni = 0usize;
    let mut star_pi: Option<usize> = None;
    let mut star_ni = 0usize;

    while ni < name.len() {
        if pi < pattern.len()
            && (pattern[pi] == b'?' || pattern[pi] == name[ni] || pattern[pi] == b'[')
        {
            if pattern[pi] == b'[' {
                let mut end = pi + 1;
                while end < pattern.len() && pattern[end] != b']' {
                    end += 1;
                }
                if end >= pattern.len() {
                    if name[ni] != b'[' {
                        if let Some(spi) = star_pi {
                            pi = spi + 1;
                            star_ni += 1;
                            ni = star_ni;
                            continue;
                        }
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
                    if let Some(spi) = star_pi {
                        pi = spi + 1;
                        star_ni += 1;
                        ni = star_ni;
                        continue;
                    }
                    return false;
                }
                ni += 1;
                pi = end + 1;
            } else {
                ni += 1;
                pi += 1;
            }
        } else if pi < pattern.len() && pattern[pi] == b'*' {
            star_pi = Some(pi);
            star_ni = ni;
            pi += 1;
        } else if let Some(spi) = star_pi {
            pi = spi + 1;
            star_ni += 1;
            ni = star_ni;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == b'*' {
        pi += 1;
    }
    pi == pattern.len()
}
