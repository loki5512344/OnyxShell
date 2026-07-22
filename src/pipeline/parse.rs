use super::{Pipeline, Segment, MAX_ARGS_PER, MAX_SEGMENTS};
use crate::io;

pub unsafe fn collect_seg_args(line: &[u8], seg: &Segment) -> usize {
    static mut G_ARGS_BUF: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
    for i in 0..seg.n_args {
        let (off, len) = seg.args[i];
        let n = len.min(63);
        for j in 0..n {
            G_ARGS_BUF[i][j] = line[off + j];
        }
        G_ARGS_BUF[i][n] = 0;
    }
    seg.n_args
}

pub fn parse(line: &[u8]) -> Pipeline {
    let mut p = Pipeline {
        segments: [Segment {
            args: [(0, 0); MAX_ARGS_PER],
            n_args: 0,
        }; MAX_SEGMENTS],
        n_segments: 0,
        stdout_file: (0, 0),
        stdin_file: (0, 0),
        background: false,
    };

    let mut i = 0;
    let len = line.len();
    while i < len && p.n_segments < MAX_SEGMENTS {
        while i < len && (io::is_whitespace(line[i]) || line[i] == b'|') {
            i += 1;
        }
        if i >= len {
            break;
        }
        let seg_idx = p.n_segments;
        p.n_segments += 1;
        let mut n_args = 0usize;
        while i < len && line[i] != b'|' && n_args < MAX_ARGS_PER {
            while i < len && io::is_whitespace(line[i]) {
                i += 1;
            }
            if i >= len || line[i] == b'|' {
                break;
            }

            if line[i] == b'>' || line[i] == b'<' {
                let op = line[i];
                i += 1;
                while i < len && io::is_whitespace(line[i]) {
                    i += 1;
                }
                let start = i;
                while i < len && !io::is_whitespace(line[i]) && line[i] != b'|' {
                    i += 1;
                }
                if op == b'>' {
                    p.stdout_file = (start, i - start);
                } else {
                    p.stdin_file = (start, i - start);
                }
                continue;
            }

            let start = i;
            while i < len && !io::is_whitespace(line[i]) && line[i] != b'|' {
                i += 1;
            }
            p.segments[seg_idx].args[n_args] = (start, i - start);
            n_args += 1;
        }
        p.segments[seg_idx].n_args = n_args;
    }

    if p.n_segments > 0 {
        let last = &mut p.segments[p.n_segments - 1];
        if last.n_args > 0 {
            let (off, len) = last.args[last.n_args - 1];
            if len == 1 && line[off] == b'&' {
                last.n_args -= 1;
                p.background = true;
            }
        }
    }

    p
}
