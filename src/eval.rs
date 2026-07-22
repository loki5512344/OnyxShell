use crate::repl::LINE_MAX;
use crate::{commands, features, io, pipeline};

pub fn has_op(s: &[u8]) -> bool {
    s.iter().any(|&b| b == b'|' || b == b'>' || b == b'<')
}

pub unsafe fn eval_line(raw: &[u8]) {
    if raw.is_empty() {
        return;
    }

    let hist = features::history_expand(raw);
    let s = hist.as_slice();
    if s.is_empty() {
        return;
    }

    if has_op(s) {
        static mut G_HIST: [u8; LINE_MAX] = [0u8; LINE_MAX];
        let n = s.len().min(LINE_MAX - 1);
        for j in 0..n {
            G_HIST[j] = s[j];
        }
        G_HIST[n] = 0;
        features::history_push(&G_HIST[..n]);
        let p = pipeline::parse(&G_HIST[..n]);
        pipeline::execute(&G_HIST[..n], &p);
        return;
    }

    let expanded = features::expand_tilde(s);
    let expanded = features::expand_vars(expanded.as_slice());
    let s = expanded.as_slice();
    if s.is_empty() {
        return;
    }

    if has_op(s) {
        static mut G_EXPANDED: [u8; LINE_MAX] = [0u8; LINE_MAX];
        let n = s.len().min(LINE_MAX - 1);
        for j in 0..n {
            G_EXPANDED[j] = s[j];
        }
        G_EXPANDED[n] = 0;
        features::history_push(&G_EXPANDED[..n]);
        let p = pipeline::parse(&G_EXPANDED[..n]);
        pipeline::execute(&G_EXPANDED[..n], &p);
        return;
    }

    features::history_push(s);

    static mut G_TOKEN_OFFSETS: [(usize, usize); 16] = [(0, 0); 16];
    let ntok = io::tokenize(s, &mut G_TOKEN_OFFSETS);
    if ntok == 0 {
        return;
    }

    static mut G_EXPANDED_ARGS: [[u8; 128]; 32] = [[0u8; 128]; 32];
    static mut G_ARGS: [&[u8]; 32] = [&[]; 32];
    let mut n_args = 0usize;
    for ti in 0..ntok {
        let (off, len) = G_TOKEN_OFFSETS[ti];
        let tok = &s[off..off + len];
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
