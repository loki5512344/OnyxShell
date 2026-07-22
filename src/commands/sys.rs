use crate::features;
use crate::io;
use crate::syscalls;

pub(crate) fn cmd_help(_args: &[&[u8]]) {
    io::write_line("OnyxShell — built-in commands");
    io::write_line("");
    io::write_line("File operations:");
    io::write_line("  ls [path] [-l]   list directory contents");
    io::write_line("  cat <file>       print file contents");
    io::write_line("  cp <src> <dst>   copy file");
    io::write_line("  mv <src> <dst>   move or rename file");
    io::write_line("  rm <file>        remove file");
    io::write_line("  mkdir <dir>      create directory");
    io::write_line("  touch <file>     create empty file");
    io::write_line("  stat <file>      show file information");
    io::write_line("");
    io::write_line("Navigation:");
    io::write_line("  cd [path]        change directory (default: /)");
    io::write_line("  pwd              print working directory");
    io::write_line("");
    io::write_line("System:");
    io::write_line("  echo [text]      print text");
    io::write_line("  export [VAR=val] set environment variable");
    io::write_line("  set              list environment variables");
    io::write_line("  unset VAR        remove environment variable");
    io::write_line("  whoami           show current user and ring");
    io::write_line("  uname            show system information");
    io::write_line("  date             show current time");
    io::write_line("  ver              show shell version");
    io::write_line("  clear            clear the screen");
    io::write_line("");
    io::write_line("Process:");
    io::write_line("  exec <path> [args]  replace shell with binary");
    io::write_line("  run <path> [args]   run binary as child (root only)");
    io::write_line("  jobs                list background jobs");
    io::write_line("  fg %<jobid>         bring job to foreground");
    io::write_line("  bg %<jobid>         continue job in background");
    io::write_line("  exit                exit the shell");
    io::write_line("");
    io::write_line("Note: rm, mkdir, cp, mv, touch require root (ring 1).");
}

pub(crate) fn cmd_echo(args: &[&[u8]]) {
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            io::write_byte(b' ');
        }
        io::write_raw(a);
    }
    io::newline();
}

pub(crate) fn cmd_export(args: &[&[u8]]) {
    if args.is_empty() {
        unsafe {
            features::env_list();
        }
        return;
    }
    for a in args {
        if let Some(eq) = a.iter().position(|&b| b == b'=') {
            let key = &a[..eq];
            let val = &a[eq + 1..];
            if !key.is_empty() {
                unsafe {
                    features::env_set(key, val);
                }
            }
        }
    }
}

pub(crate) fn cmd_set(_args: &[&[u8]]) {
    unsafe {
        features::env_list();
    }
}

pub(crate) fn cmd_unset(args: &[&[u8]]) {
    for a in args {
        unsafe {
            features::env_unset(a);
        }
    }
}

pub(crate) fn cmd_whoami(_args: &[&[u8]]) {
    let uid = unsafe { syscalls::getuid() };
    let ring = unsafe { syscalls::getring() };

    let user_str: &[u8] = if uid == 0 { b"root" } else { b"user" };
    let ring_str: &[u8] = match ring {
        0 => b"kernel",
        1 => b"root",
        2 => b"user",
        _ => b"unknown",
    };

    io::write_raw(user_str);
    io::write_raw(b" (uid=");
    io::write_i64(uid);
    io::write_raw(b", ring=");
    io::write_raw(ring_str);
    io::write_raw(b")\n");
}

pub(crate) fn cmd_uname(_args: &[&[u8]]) {
    let mut buf = [0u8; 390];
    let ret = unsafe { syscalls::uname(buf.as_mut_ptr()) };
    if ret < 0 {
        io::write_error_errno("uname", ret);
        return;
    }

    let labels: [(&[u8], usize); 5] = [
        (b"sysname", 0),
        (b"nodename", 65),
        (b"release", 130),
        (b"version", 195),
        (b"machine", 260),
    ];
    for &(label, off) in &labels {
        let mut end = off;
        while end < off + 65 && buf[end] != 0 {
            end += 1;
        }
        io::write_raw(b"  ");
        io::write_raw(label);
        io::write_raw(b": ");
        io::write_raw(&buf[off..end]);
        io::newline();
    }
}

pub(crate) fn cmd_date(_args: &[&[u8]]) {
    let mut ts = [0u64; 2];
    let ret = unsafe { syscalls::clock_gettime(0, ts.as_mut_ptr()) };
    if ret < 0 {
        io::write_error_errno("date", ret);
        return;
    }

    io::write_raw(b"epoch: ");
    io::write_u64(ts[0]);
    io::write_raw(b" sec, ");
    io::write_u64(ts[1]);
    io::write_raw(b" nsec\n");
}

pub(crate) fn cmd_clear(_args: &[&[u8]]) {
    io::write_raw(b"\x1b[2J\x1b[H");
}

pub(crate) fn cmd_exit(_args: &[&[u8]]) {
    io::write_line("logout");
    unsafe {
        syscalls::exit(0);
    }
}

pub(crate) fn cmd_ver(_args: &[&[u8]]) {
    io::write_line("OnyxShell v0.2.0 — built-in command shell for OnyxOS");
    io::write_line("Copyright (c) 2024-2025 loki5512344");
    io::write_line("License: GPL-3.0-or-later");
}
