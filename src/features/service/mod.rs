//! Tab completion for built-in commands and filesystem paths.

use crate::features::buffer::{line_to_vec, Vec};
use crate::features::service::glob::{entry_startswith, scan_dir_entries, split_dir_and_prefix};
use crate::io;

pub struct TabResult {
    pub line: Vec<u8>,
    pub cursor: usize,
    pub printed: bool,
}

const BUILTINS: &[&[u8]] = &[
    b"ls", b"cat", b"cp", b"mv", b"rm", b"mkdir", b"touch", b"stat", b"cd", b"pwd", b"echo",
    b"whoami", b"uname", b"date", b"clear", b"help", b"exit", b"exec", b"run", b"ver", b"export",
    b"set", b"unset", b"jobs", b"fg", b"bg", b"source",
];

pub unsafe fn tab_complete(line: &[u8], cursor: usize) -> TabResult {
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
        let mut new_line: Vec<u8> = Vec::new();
        new_line.extend_from_slice(&line[..tok_start]);
        new_line.extend_from_slice(&matches[0][..common]);
        new_line.extend_from_slice(&line[cursor..]);
        let new_cursor = tok_start + common;
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

pub mod glob;
pub mod jobs;
