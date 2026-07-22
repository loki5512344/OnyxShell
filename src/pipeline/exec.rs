use super::{Pipeline, MAX_ARGS_PER, MAX_SEGMENTS};
use crate::{commands, features, io, syscalls};
pub fn is_inprocess_builtin(cmd: &[u8]) -> bool {
    matches!(cmd, b"cd" | b"exit" | b"pwd" | b"help" | b"whoami" | b"uname" | b"ver" | b"clear" | b"date")
}
pub unsafe fn execute(line: &[u8], p: &Pipeline) {
    features::job_reap();
    if p.n_segments == 1 && p.stdout_file.1 == 0 && p.stdin_file.1 == 0 {
        let seg = &p.segments[0];
        if seg.n_args == 0 { return; }
        static mut G_ARGS: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
        static mut G_ARGS_BUF: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
        for i in 0..seg.n_args {
            let (off, len) = seg.args[i];
            let n = len.min(63);
            for j in 0..n { G_ARGS_BUF[i][j] = line[off + j]; }
            G_ARGS_BUF[i][n] = 0;
            G_ARGS[i] = &G_ARGS_BUF[i][..n];
        }
        if p.background {
            let pid = syscalls::fork();
            if pid < 0 { io::write_error_errno("fork", pid); return; }
            if pid == 0 { commands::dispatch(&G_ARGS[..seg.n_args]); syscalls::exit(0); }
            let job_id = features::job_add(pid as i32);
            io::write_raw(b"["); io::write_u64(job_id as u64); io::write_raw(b"] "); io::write_i64(pid); io::newline();
            return;
        }
        commands::dispatch(&G_ARGS[..seg.n_args]);
        return;
    }
    let stdout_fd: i64 = if p.stdout_file.1 > 0 {
        let (off, len) = p.stdout_file;
        static mut G_OUT_PATH: [u8; 256] = [0u8; 256];
        let n = len.min(255);
        for j in 0..n { G_OUT_PATH[j] = line[off + j]; }
        G_OUT_PATH[n] = 0;
        syscalls::open(G_OUT_PATH.as_ptr(), (syscalls::O_WRONLY | syscalls::O_CREAT | syscalls::O_TRUNC) as u64, 0)
    } else { 1 };
    let stdin_fd: i64 = if p.stdin_file.1 > 0 {
        let (off, len) = p.stdin_file;
        static mut G_IN_PATH: [u8; 256] = [0u8; 256];
        let n = len.min(255);
        for j in 0..n { G_IN_PATH[j] = line[off + j]; }
        G_IN_PATH[n] = 0;
        syscalls::open(G_IN_PATH.as_ptr(), syscalls::O_RDONLY as u64, 0)
    } else { 0 };
    if stdout_fd < 0 { io::write_error_errno("open (redirect)", stdout_fd); return; }
    if stdin_fd < 0 { io::write_error_errno("open (redirect)", stdin_fd); if stdout_fd > 2 { syscalls::close(stdout_fd as u64); } return; }
    if p.n_segments == 1 {
        let seg = &p.segments[0];
        if seg.n_args == 0 { return; }
        let (a_off, a_len) = seg.args[0];
        if is_inprocess_builtin(&line[a_off..a_off + a_len]) {
            static mut G_ARGS2: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
            static mut G_ARGS_BUF2: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
            for i in 0..seg.n_args {
                let (off, len) = seg.args[i];
                let n = len.min(63);
                for j in 0..n { G_ARGS_BUF2[i][j] = line[off + j]; }
                G_ARGS_BUF2[i][n] = 0;
                G_ARGS2[i] = &G_ARGS_BUF2[i][..n];
            }
            commands::dispatch(&G_ARGS2[..seg.n_args]);
            if stdout_fd > 2 { syscalls::close(stdout_fd as u64); }
            if stdin_fd > 2 { syscalls::close(stdin_fd as u64); }
            return;
        }
    }
    let mut prev_read_fd: i64 = stdin_fd;
    let mut last_pid: i64 = 0;
    let mut spawned_pids: [i64; MAX_SEGMENTS] = [0i64; MAX_SEGMENTS];
    let mut spawned_count: usize = 0;
    let mut parent_close: [(i64, bool); MAX_SEGMENTS] = [(-1, false); MAX_SEGMENTS];
    for seg_idx in 0..p.n_segments {
        let seg = &p.segments[seg_idx];
        if seg.n_args == 0 { continue; }
        let is_last = seg_idx == p.n_segments - 1;
        let mut pipe_fds = [0u64; 2];
        let cur_read_fd = prev_read_fd;
        let cur_write_fd: i64;
        if is_last { cur_write_fd = stdout_fd; }
        else {
            if syscalls::pipe(pipe_fds.as_mut_ptr()) < 0 { io::write_error("pipe failed"); return; }
            cur_write_fd = pipe_fds[1] as i64;
            prev_read_fd = pipe_fds[0] as i64;
        }
        let pid = syscalls::fork();
        if pid < 0 {
            io::write_error_errno("fork", pid);
            if cur_write_fd > 2 { syscalls::close(cur_write_fd as u64); }
            if cur_read_fd > 2 && cur_read_fd != stdin_fd { syscalls::close(cur_read_fd as u64); }
            return;
        }
        if pid == 0 {
            if cur_read_fd != 0 { syscalls::close(0); let _ = syscalls::dup(cur_read_fd as u64); }
            if cur_write_fd != 1 { syscalls::close(1); let _ = syscalls::dup(cur_write_fd as u64); }
            static mut G_ARGS3: [&[u8]; MAX_ARGS_PER] = [&[]; MAX_ARGS_PER];
            static mut G_ARGS_BUF3: [[u8; 64]; MAX_ARGS_PER] = [[0u8; 64]; MAX_ARGS_PER];
            for i in 0..seg.n_args {
                let (off, len) = seg.args[i];
                let n = len.min(63);
                for j in 0..n { G_ARGS_BUF3[i][j] = line[off + j]; }
                G_ARGS_BUF3[i][n] = 0;
                G_ARGS3[i] = &G_ARGS_BUF3[i][..n];
            }
            commands::dispatch(&G_ARGS3[..seg.n_args]);
            syscalls::exit(0);
        }
        spawned_pids[spawned_count] = pid;
        parent_close[spawned_count] = (
            if cur_read_fd > 2 && cur_read_fd != stdin_fd { cur_read_fd }
            else if cur_write_fd > 2 && cur_write_fd != stdout_fd { cur_write_fd }
            else { -1 },
            cur_write_fd > 2 && cur_write_fd != stdout_fd,
        );
        spawned_count += 1;
        last_pid = pid;
    }
    for i in 0..spawned_count { let (fd, _) = parent_close[i]; if fd > 2 { syscalls::close(fd as u64); } }
    if !p.background {
        for i in 0..spawned_count { let pid = spawned_pids[i]; if pid > 0 { let mut s: i32 = 0; syscalls::waitpid(pid as u64, &mut s, 0); } }
    }
    if p.background && last_pid > 0 {
        let job_id = features::job_add(last_pid as i32);
        io::write_raw(b"["); io::write_u64(job_id as u64); io::write_raw(b"] "); io::write_i64(last_pid); io::newline();
    }
}
