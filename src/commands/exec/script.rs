use crate::features;
use crate::io;
use crate::path;
use crate::pipeline;
use crate::syscalls;

use super::parse_job_id;

const SCRIPT_DEPTH_MAX: u8 = 8;
static mut SCRIPT_DEPTH: u8 = 0;

pub(crate) fn cmd_source(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("source: missing file operand (try 'help')");
        return;
    }
    do_script(args[0]);
}

pub fn do_script(input: &[u8]) {
    unsafe {
        if SCRIPT_DEPTH >= SCRIPT_DEPTH_MAX {
            io::write_error("source: max recursion depth exceeded");
            return;
        }
        SCRIPT_DEPTH += 1;
    }
    let result = do_script_inner(input);
    unsafe {
        SCRIPT_DEPTH -= 1;
    }
    result
}

fn do_script_inner(input: &[u8]) {
    let mut abs = [0u8; path::PATH_MAX];
    let len = path::resolve(input, &mut abs);
    if len == 0 {
        io::write_error("source: path too long");
        return;
    }

    let fd = unsafe { syscalls::open(abs.as_ptr(), syscalls::O_RDONLY as u64, 0) };
    if fd < 0 {
        io::write_error_errno("source", fd);
        return;
    }

    let mut line_buf = [0u8; crate::commands::LINE_MAX];
    let mut line_pos = 0usize;
    loop {
        let mut c = [0u8; 1];
        let n = unsafe { syscalls::read_fd(fd as u64, c.as_mut_ptr(), 1) };
        if n <= 0 {
            break;
        }
        if c[0] == b'\n' {
            if line_pos > 0 && line_buf[0] != b'#' {
                execute_line(&line_buf[..line_pos]);
            }
            line_pos = 0;
        } else if c[0] != b'\r' && line_pos < crate::commands::LINE_MAX - 1 {
            line_buf[line_pos] = c[0];
            line_pos += 1;
        }
    }
    if line_pos > 0 && line_buf[0] != b'#' {
        execute_line(&line_buf[..line_pos]);
    }

    unsafe {
        syscalls::close(fd as u64);
    }
}

fn execute_line(line: &[u8]) {
    let expanded = unsafe { features::expand_tilde(line) };
    let expanded = unsafe { features::expand_vars(expanded.as_slice()) };
    let expanded_slice = expanded.as_slice();

    let has_pipe_or_redirect = expanded_slice
        .iter()
        .any(|&b| b == b'|' || b == b'>' || b == b'<');
    if has_pipe_or_redirect {
        static mut G_EXPANDED: [u8; 256] = [0u8; 256];
        let n = expanded_slice.len().min(255);
        unsafe {
            for j in 0..n {
                G_EXPANDED[j] = expanded_slice[j];
            }
            G_EXPANDED[n] = 0;
            let p = pipeline::parse(&G_EXPANDED[..n]);
            pipeline::execute(&G_EXPANDED[..n], &p);
        }
        return;
    }

    static mut G_TOKEN_OFFSETS: [(usize, usize); 16] = [(0, 0); 16];
    let ntok = io::tokenize(expanded_slice, unsafe { &mut G_TOKEN_OFFSETS });
    if ntok == 0 {
        return;
    }

    static mut G_EXPANDED_ARGS: [[u8; 128]; 32] = [[0u8; 128]; 32];
    static mut G_ARGS: [&[u8]; 32] = [&[]; 32];
    let mut n_args = 0usize;
    for ti in 0..ntok {
        let (off, len) = unsafe { G_TOKEN_OFFSETS[ti] };
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
            unsafe {
                G_EXPANDED_ARGS[n_args] = exp;
                G_ARGS[n_args] = &G_EXPANDED_ARGS[n_args][..elen];
            }
            n_args += 1;
        }
        if n_args >= 32 {
            break;
        }
    }

    crate::commands::dispatch(unsafe { &G_ARGS[..n_args] });
}

pub(crate) fn cmd_jobs(_args: &[&[u8]]) {
    unsafe {
        features::job_list();
    }
}

pub(crate) fn cmd_fg(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("fg: usage: fg %<jobid>");
        return;
    }
    let job_id = parse_job_id(args[0]);
    if job_id == 0 {
        io::write_error("fg: invalid job specifier (use %<number>)");
        return;
    }
    unsafe {
        if let Some((id, pid, _running)) = features::job_find_by_id(job_id) {
            let mut status: i32 = 0;
            syscalls::waitpid(pid as u64, &mut status, 0);
            io::write_raw(b"[");
            io::write_u64(id as u64);
            io::write_raw(b"] ");
            io::write_i64(pid as i64);
            io::write_raw(b" Done\n");
            features::job_remove_by_id(id);
        } else {
            io::write_error("fg: job not found");
        }
    }
}

pub(crate) fn cmd_bg(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("bg: usage: bg %<jobid>");
        return;
    }
    let job_id = parse_job_id(args[0]);
    if job_id == 0 {
        io::write_error("bg: invalid job specifier (use %<number>)");
        return;
    }
    unsafe {
        if let Some((id, pid, running)) = features::job_find_by_id(job_id) {
            if !running {
                syscalls::kill(pid, syscalls::SIGCONT);
                features::job_set_running(id, true);
            }
            io::write_raw(b"[");
            io::write_u64(id as u64);
            io::write_raw(b"] ");
            io::write_i64(pid as i64);
            io::newline();
        } else {
            io::write_error("bg: job not found");
        }
    }
}
