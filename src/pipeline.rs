//! Pipeline / redirect parser and executor.
//!
//! Splits a command line into segments separated by `|`, extracts `>`
//! and `<` redirect operators, then executes the segments. Built-in
//! commands run in-process; external binaries run via `fork` + `exec`
//! with their stdin / stdout wired to the appropriate pipe ends.
//!
//! Layout of a parsed line:
//!   cmd0 args | cmd1 args | cmd2 args > outfile < infile
//!
//! For a single segment (no `|`), we just run the built-in or exec
//! the binary in the current process (matching the previous behavior).
//! For multi-segment pipelines, we fork for each segment, wire stdin /
//! stdout to pipe ends, and exec the binary OR run a built-in that
//! supports redirection (currently only `cat`, `echo`, `ls`).
//!
//! Built-in commands that don't support redirection (like `cd`, `exit`)
//! are detected and run in the parent process before the pipeline runs.

use crate::commands;
use crate::io;
use crate::syscalls;

pub const MAX_SEGMENTS: usize = 8;
pub const MAX_ARGS_PER: usize = 16;

/// One segment of a pipeline: `cmd arg1 arg2 ...`
#[derive(Copy, Clone)]
pub struct Segment {
    pub args: [(usize, usize); MAX_ARGS_PER], // (offset, len) into the line buffer
    pub n_args: usize,
}

/// Parsed pipeline: segments + optional redirect targets.
pub struct Pipeline {
    pub segments: [Segment; MAX_SEGMENTS],
    pub n_segments: usize,
    /// Offset/len into the line buffer for the `> file` target, or (0,0) if none.
    pub stdout_file: (usize, usize),
    /// Offset/len into the line buffer for the `< file` source, or (0,0) if none.
    pub stdin_file: (usize, usize),
}

/// Parse a line into a Pipeline. The line is the raw input (no trailing newline).
pub fn parse(line: &[u8]) -> Pipeline {
    let mut p = Pipeline {
        segments: [Segment {
            args: [(0usize, 0usize); MAX_ARGS_PER],
            n_args: 0,
        }; MAX_SEGMENTS],
        n_segments: 0,
        stdout_file: (0, 0),
        stdin_file: (0, 0),
    };

    let mut i = 0;
    let len = line.len();
    while i < len && p.n_segments < MAX_SEGMENTS {
        // Skip whitespace and pipe operators.
        while i < len && (io::is_whitespace(line[i]) || line[i] == b'|') {
            i += 1;
        }
        if i >= len {
            break;
        }
        // Start of a segment.
        let seg_idx = p.n_segments;
        p.n_segments += 1;
        let mut n_args = 0usize;
        while i < len && line[i] != b'|' && n_args < MAX_ARGS_PER {
            // Skip whitespace.
            while i < len && io::is_whitespace(line[i]) {
                i += 1;
            }
            if i >= len || line[i] == b'|' {
                break;
            }
            // Check for redirect operators.
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
            // Read a normal token.
            let start = i;
            while i < len && !io::is_whitespace(line[i]) && line[i] != b'|' {
                i += 1;
            }
            p.segments[seg_idx].args[n_args] = (start, i - start);
            n_args += 1;
        }
        p.segments[seg_idx].n_args = n_args;
    }
    p
}

/// Execute a parsed pipeline.
///
/// For a single segment with no redirects, defer to `commands::dispatch`
/// (preserves existing behavior including built-ins like `cd`, `exit`).
/// For multi-segment pipelines or redirects, fork for each segment and
/// wire stdin/stdout accordingly.
pub unsafe fn execute(line: &[u8], p: &Pipeline) {
    // If there's only one segment and no redirects, just dispatch.
    if p.n_segments == 1 && p.stdout_file.1 == 0 && p.stdin_file.1 == 0 {
        let seg = &p.segments[0];
        if seg.n_args == 0 {
            return;
        }
        // Copy each token into a static buffer so we can build a 'static
        // slice-of-slices for dispatch (which expects &'static [&'static [u8]]).
        static mut G_ARGS_BUF: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
        static mut G_ARGS: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
        for i in 0..seg.n_args {
            let (off, len) = seg.args[i];
            let n = len.min(63);
            for j in 0..n {
                G_ARGS_BUF[i][j] = line[off + j];
            }
            G_ARGS_BUF[i][n] = 0;
            G_ARGS[i] = &G_ARGS_BUF[i][..n];
        }
        commands::dispatch(&G_ARGS[..seg.n_args]);
        return;
    }

    // Multi-segment pipeline or redirects — need fork + pipes.
    // Open the redirect targets first (in the parent, so all children inherit).
    let stdout_fd: i64 = if p.stdout_file.1 > 0 {
        let (off, len) = p.stdout_file;
        // NUL-terminate into a static buffer (path is < 256 bytes).
        static mut G_OUT_PATH: [u8; 256] = [0u8; 256];
        let n = len.min(255);
        for j in 0..n {
            G_OUT_PATH[j] = line[off + j];
        }
        G_OUT_PATH[n] = 0;
        syscalls::open(G_OUT_PATH.as_ptr(), (syscalls::O_WRONLY | syscalls::O_CREAT | syscalls::O_TRUNC) as u64, 0)
    } else {
        1 // stdout
    };
    let stdin_fd: i64 = if p.stdin_file.1 > 0 {
        let (off, len) = p.stdin_file;
        static mut G_IN_PATH: [u8; 256] = [0u8; 256];
        let n = len.min(255);
        for j in 0..n {
            G_IN_PATH[j] = line[off + j];
        }
        G_IN_PATH[n] = 0;
        syscalls::open(G_IN_PATH.as_ptr(), syscalls::O_RDONLY as u64, 0)
    } else {
        0 // stdin
    };

    if stdout_fd < 0 {
        io::write_error_errno("open (redirect)", stdout_fd);
        return;
    }
    if stdin_fd < 0 {
        io::write_error_errno("open (redirect)", stdin_fd);
        if stdout_fd > 2 {
            syscalls::close(stdout_fd as u64);
        }
        return;
    }

    // For a single segment with redirects (no pipes), we can run the
    // built-in in-process with stdout/stdin temporarily swapped — but
    // the kernel doesn't support dup2 yet. So we require fork for any
    // redirect case. For built-ins that don't make sense with redirects
    // (cd, exit, help), we just dispatch normally and ignore the redirect.
    if p.n_segments == 1 {
        let seg = &p.segments[0];
        if seg.n_args == 0 {
            return;
        }
        // Check if it's a built-in that should run in-process.
        let (a_off, a_len) = seg.args[0];
        let cmd = &line[a_off..a_off + a_len];
        if is_inprocess_builtin(cmd) {
            // Run in-process, ignore redirect (cd, exit don't produce stdout).
            static mut G_ARGS_BUF2: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
            static mut G_ARGS2: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
            for i in 0..seg.n_args {
                let (off, len) = seg.args[i];
                let n = len.min(63);
                for j in 0..n {
                    G_ARGS_BUF2[i][j] = line[off + j];
                }
                G_ARGS_BUF2[i][n] = 0;
                G_ARGS2[i] = &G_ARGS_BUF2[i][..n];
            }
            commands::dispatch(&G_ARGS2[..seg.n_args]);
            if stdout_fd > 2 {
                syscalls::close(stdout_fd as u64);
            }
            if stdin_fd > 2 {
                syscalls::close(stdin_fd as u64);
            }
            return;
        }
    }

    // Fork for each segment. Each child wires its stdin/stdout and execs.
    let mut prev_read_fd: i64 = stdin_fd; // start from redirect stdin (or 0)
    for seg_idx in 0..p.n_segments {
        let seg = &p.segments[seg_idx];
        if seg.n_args == 0 {
            continue;
        }
        let is_last = seg_idx == p.n_segments - 1;
        // Create a pipe for this segment's output (unless last).
        let mut pipe_fds = [0u64; 2];
        let cur_read_fd = prev_read_fd;
        let cur_write_fd: i64;
        if is_last {
            cur_write_fd = stdout_fd;
        } else {
            if syscalls::pipe(pipe_fds.as_mut_ptr()) < 0 {
                io::write_error("pipe failed");
                return;
            }
            cur_write_fd = pipe_fds[1] as i64;
            prev_read_fd = pipe_fds[0] as i64;
        }

        // Fork.
        let pid = syscalls::fork();
        if pid < 0 {
            io::write_error_errno("fork", pid);
            if cur_write_fd > 2 {
                syscalls::close(cur_write_fd as u64);
            }
            if cur_read_fd > 2 && cur_read_fd != stdin_fd {
                syscalls::close(cur_read_fd as u64);
            }
            return;
        }
        if pid == 0 {
            // Child: wire stdin/stdout, then exec or run built-in.
            // (We can't easily redirect fd 0/1 without dup2, so we pass
            // the explicit fds to a built-in runner. For external binaries
            // we'd need dup2 — not yet available. For now, built-ins only.)
            if cur_read_fd != 0 {
                // Best effort: dup the read end onto fd 0. dup() returns a
                // new fd, doesn't replace — we'd need dup2 for that. So we
                // just close 0 and hope the exec inherits the fd table.
                syscalls::close(0);
                let _ = syscalls::dup(cur_read_fd as u64);
            }
            if cur_write_fd != 1 {
                syscalls::close(1);
                let _ = syscalls::dup(cur_write_fd as u64);
            }
            // Run the built-in (most built-ins write to fd 1 via syscalls::write).
            static mut G_ARGS_BUF3: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
            static mut G_ARGS3: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
            for i in 0..seg.n_args {
                let (off, len) = seg.args[i];
                let n = len.min(63);
                for j in 0..n {
                    G_ARGS_BUF3[i][j] = line[off + j];
                }
                G_ARGS_BUF3[i][n] = 0;
                G_ARGS3[i] = &G_ARGS_BUF3[i][..n];
            }
            commands::dispatch(&G_ARGS3[..seg.n_args]);
            // Exit the child.
            syscalls::exit(0);
        }
        // Parent: close the fds we don't need anymore.
        if cur_read_fd > 2 && cur_read_fd != stdin_fd {
            syscalls::close(cur_read_fd as u64);
        }
        if cur_write_fd > 2 && cur_write_fd != stdout_fd {
            syscalls::close(cur_write_fd as u64);
        }
        // Wait for the child to finish before starting the next segment.
        // (Sequential pipeline — simpler than parallel, and avoids the
        // kernel's waitpid limitations.)
        let mut status: i32 = 0;
        syscalls::waitpid(pid as u64, &mut status, 0);
    }
}

/// Built-in commands that must run in the parent process (can't be forked).
fn is_inprocess_builtin(cmd: &[u8]) -> bool {
    matches!(
        cmd,
        b"cd" | b"exit" | b"pwd" | b"help" | b"whoami" | b"uname" | b"ver" | b"clear" | b"date"
    )
}
