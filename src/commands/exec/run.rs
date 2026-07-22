use crate::features;
use crate::io;
use crate::path;
use crate::syscalls;

use super::{build_argv, build_envp, build_shebang_argv, check_shebang, resolve_cmd_path};

pub(crate) fn cmd_exec(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("exec: missing path (usage: exec <path> [args...])");
        return;
    }

    let mut path_buf = [0u8; path::PATH_MAX];
    let len = resolve_cmd_path(args[0], &mut path_buf);
    if len == 0 {
        io::write_error("exec: path too long");
        return;
    }

    if let Some(interp_buf) = check_shebang(path_buf.as_ptr()) {
        let interp_len = interp_buf.iter().position(|&b| b == 0).unwrap_or(256);
        if interp_len > 0 && &interp_buf[..interp_len] == b"/bin/osh" {
            crate::commands::exec::script::do_script(&path_buf[..len]);
            unsafe {
                syscalls::exit(0);
            }
        }

        let mut argv_strs: [[u8; path::PATH_MAX]; crate::commands::MAX_ARGS] =
            [[0; path::PATH_MAX]; crate::commands::MAX_ARGS];
        let mut argv_ptrs = [0u64; crate::commands::MAX_ARGS + 1];
        let target = &interp_buf[..interp_len];
        let argc = build_shebang_argv(
            target,
            &path_buf[..len],
            args,
            &mut argv_strs,
            &mut argv_ptrs,
        );
        if argc == 0 {
            return;
        }

        let mut envp_strs =
            [[0u8; features::ENV_KEY_MAX + features::ENV_VAL_MAX + 2]; features::ENV_MAX];
        let mut envp_ptrs = [0u64; features::ENV_MAX + 1];
        build_envp(&mut envp_strs, &mut envp_ptrs);

        let ret = unsafe {
            syscalls::execve(
                argv_strs[0].as_ptr(),
                argv_ptrs.as_ptr(),
                envp_ptrs.as_ptr(),
            )
        };
        io::write_error_errno("exec", ret);
        return;
    }

    let mut argv_strs: [[u8; path::PATH_MAX]; crate::commands::MAX_ARGS] =
        [[0; path::PATH_MAX]; crate::commands::MAX_ARGS];
    let mut argv_ptrs = [0u64; crate::commands::MAX_ARGS + 1];
    let argc = build_argv(args, &mut argv_strs, &mut argv_ptrs);
    if argc == 0 {
        io::write_error("exec: argument too long");
        return;
    }

    let mut envp_strs =
        [[0u8; features::ENV_KEY_MAX + features::ENV_VAL_MAX + 2]; features::ENV_MAX];
    let mut envp_ptrs = [0u64; features::ENV_MAX + 1];
    build_envp(&mut envp_strs, &mut envp_ptrs);

    let ret =
        unsafe { syscalls::execve(path_buf.as_ptr(), argv_ptrs.as_ptr(), envp_ptrs.as_ptr()) };
    io::write_error_errno("exec", ret);
}

pub(crate) fn cmd_run(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("run: missing path (usage: run <path> [args...])");
        return;
    }

    let mut path_buf = [0u8; path::PATH_MAX];
    let len = resolve_cmd_path(args[0], &mut path_buf);
    if len == 0 {
        io::write_error("run: path too long");
        return;
    }

    if let Some(interp_buf) = check_shebang(path_buf.as_ptr()) {
        let interp_len = interp_buf.iter().position(|&b| b == 0).unwrap_or(256);
        let target: &[u8] = if interp_len > 0 && &interp_buf[..interp_len] == b"/bin/osh" {
            b"/bin/osh"
        } else {
            &interp_buf[..interp_len]
        };

        let mut argv_strs: [[u8; path::PATH_MAX]; crate::commands::MAX_ARGS] =
            [[0; path::PATH_MAX]; crate::commands::MAX_ARGS];
        let mut argv_ptrs = [0u64; crate::commands::MAX_ARGS + 1];
        let argc = build_shebang_argv(
            target,
            &path_buf[..len],
            args,
            &mut argv_strs,
            &mut argv_ptrs,
        );
        if argc == 0 {
            return;
        }

        let pid = unsafe { syscalls::spawn(argv_strs[0].as_ptr(), argv_ptrs.as_ptr(), 0) };
        if pid < 0 {
            io::write_error_errno("run", pid);
            return;
        }
        let mut status: i32 = 0;
        let waited = unsafe { syscalls::wait(&mut status) };
        if waited < 0 {
            io::write_error_errno("run: wait", waited);
            return;
        }
        if status != 0 {
            io::write_raw(b"osh: process exited with code ");
            io::write_i64(status as i64);
            io::newline();
        }
        return;
    }

    let mut argv_strs: [[u8; path::PATH_MAX]; crate::commands::MAX_ARGS] =
        [[0; path::PATH_MAX]; crate::commands::MAX_ARGS];
    let mut argv_ptrs = [0u64; crate::commands::MAX_ARGS + 1];
    let argc = build_argv(args, &mut argv_strs, &mut argv_ptrs);
    if argc == 0 {
        io::write_error("run: argument too long");
        return;
    }

    let pid = unsafe { syscalls::spawn(path_buf.as_ptr(), argv_ptrs.as_ptr(), 0) };
    if pid < 0 {
        io::write_error_errno("run", pid);
        return;
    }

    let mut status: i32 = 0;
    let waited = unsafe { syscalls::wait(&mut status) };
    if waited < 0 {
        io::write_error_errno("run: wait", waited);
        return;
    }

    if status != 0 {
        io::write_raw(b"osh: process exited with code ");
        io::write_i64(status as i64);
        io::newline();
    }
}
