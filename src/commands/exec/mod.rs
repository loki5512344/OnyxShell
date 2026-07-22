use crate::features;
use crate::io;
use crate::path;
use crate::syscalls;

mod run;
pub(crate) mod script;

pub(crate) use run::{cmd_exec, cmd_run};
pub(crate) use script::{cmd_bg, cmd_fg, cmd_jobs, cmd_source, do_script};
fn search_path(cmd: &[u8]) -> Option<[u8; path::PATH_MAX]> {
    if cmd.contains(&b'/') {
        return None;
    }
    unsafe {
        let path_var = features::env_get(b"PATH")?;
        let mut i = 0;
        while i < path_var.len() {
            let start = i;
            while i < path_var.len() && path_var[i] != b':' {
                i += 1;
            }
            let dir = &path_var[start..i];
            if !dir.is_empty() {
                let mut abs_dir = [0u8; path::PATH_MAX];
                let dlen = path::resolve(dir, &mut abs_dir);
                if dlen > 0 {
                    let mut full = [0u8; path::PATH_MAX];
                    let mut pos = 0;
                    for &b in &abs_dir[..dlen] {
                        if pos >= path::PATH_MAX - 1 {
                            break;
                        }
                        full[pos] = b;
                        pos += 1;
                    }
                    if pos > 0 && full[pos - 1] != b'/' {
                        if pos >= path::PATH_MAX - 1 {
                            break;
                        }
                        full[pos] = b'/';
                        pos += 1;
                    }
                    for &b in cmd {
                        if pos >= path::PATH_MAX - 1 {
                            break;
                        }
                        full[pos] = b;
                        pos += 1;
                    }
                    full[pos] = 0;
                    let mut st = [0u8; 256];
                    if syscalls::stat(full.as_ptr(), st.as_mut_ptr()) >= 0 {
                        return Some(full);
                    }
                }
            }
            if i < path_var.len() && path_var[i] == b':' {
                i += 1;
            }
        }
    }
    None
}

pub(crate) fn check_shebang(path: *const u8) -> Option<[u8; 256]> {
    let fd = unsafe { syscalls::open(path, syscalls::O_RDONLY as u64, 0) };
    if fd < 0 {
        return None;
    }

    let mut buf = [0u8; 256];
    let mut pos = 0usize;
    loop {
        let mut c = [0u8; 1];
        let n = unsafe { syscalls::read_fd(fd as u64, c.as_mut_ptr(), 1) };
        if n <= 0 || c[0] == b'\n' || pos >= 255 {
            break;
        }
        buf[pos] = c[0];
        pos += 1;
    }
    unsafe {
        syscalls::close(fd as u64);
    }

    if pos < 2 || buf[0] != b'#' || buf[1] != b'!' {
        return None;
    }

    let mut start = 2;
    while start < pos && (buf[start] == b' ' || buf[start] == b'\t') {
        start += 1;
    }
    let mut end = start;
    while end < pos && buf[end] != b' ' && buf[end] != b'\t' {
        end += 1;
    }

    if start >= end {
        return None;
    }

    let mut result = [0u8; 256];
    let n = (end - start).min(255);
    for i in 0..n {
        result[i] = buf[start + i];
    }
    Some(result)
}
pub(crate) fn parse_job_id(arg: &[u8]) -> usize {
    if arg.len() > 1 && arg[0] == b'%' {
        let mut id = 0usize;
        for &b in &arg[1..] {
            if b >= b'0' && b <= b'9' {
                id = match id
                    .checked_mul(10)
                    .and_then(|v| v.checked_add((b - b'0') as usize))
                {
                    Some(v) => v,
                    None => return 0,
                };
            } else {
                return 0;
            }
        }
        id
    } else {
        0
    }
}

fn resolve_cmd_path(cmd: &[u8], out: &mut [u8; path::PATH_MAX]) -> usize {
    if cmd.contains(&b'/') {
        path::resolve(cmd, out)
    } else if let Some(found) = search_path(cmd) {
        let flen = found.iter().position(|&b| b == 0).unwrap_or(path::PATH_MAX);
        out[..flen].copy_from_slice(&found[..flen]);
        flen
    } else {
        path::resolve(cmd, out)
    }
}
fn build_argv(
    args: &[&[u8]],
    argv_strs: &mut [[u8; path::PATH_MAX]; super::MAX_ARGS],
    argv_ptrs: &mut [u64; super::MAX_ARGS + 1],
) -> usize {
    let argc = args.len().min(super::MAX_ARGS);
    for i in 0..argc {
        let arg = args[i];
        if arg.len() >= path::PATH_MAX {
            return 0;
        }
        argv_strs[i][..arg.len()].copy_from_slice(arg);
        argv_strs[i][arg.len()] = 0;
        argv_ptrs[i] = argv_strs[i].as_ptr() as u64;
    }
    argv_ptrs[argc] = 0;
    argc
}

fn build_shebang_argv(
    target: &[u8],
    script_path: &[u8],
    args: &[&[u8]],
    argv_strs: &mut [[u8; path::PATH_MAX]; super::MAX_ARGS],
    argv_ptrs: &mut [u64; super::MAX_ARGS + 1],
) -> usize {
    let mut argc = 0usize;
    argv_strs[argc][..target.len()].copy_from_slice(target);
    argv_strs[argc][target.len()] = 0;
    argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
    argc += 1;

    argv_strs[argc][..script_path.len()].copy_from_slice(script_path);
    argv_strs[argc][script_path.len()] = 0;
    argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
    argc += 1;

    for i in 1..args.len().min(super::MAX_ARGS - 2) {
        let arg = args[i];
        if arg.len() >= path::PATH_MAX {
            break;
        }
        argv_strs[argc][..arg.len()].copy_from_slice(arg);
        argv_strs[argc][arg.len()] = 0;
        argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
        argc += 1;
    }
    argv_ptrs[argc] = 0;
    argc
}
fn build_envp(
    envp_strs: &mut [[u8; features::ENV_KEY_MAX + features::ENV_VAL_MAX + 2]; features::ENV_MAX],
    envp_ptrs: &mut [u64; features::ENV_MAX + 1],
) -> usize {
    unsafe { features::build_envp(envp_strs, envp_ptrs) }
}
