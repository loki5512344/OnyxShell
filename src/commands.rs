//! Command implementations for OnyxShell.
//!
//! Each command is a function `fn(args: &[&[u8]])` that takes a slice
//! of argument byte-slices (args[0] is the command name itself).
//!
//! ## Privilege model
//!
//! File-mutation commands (`rm`, `mkdir`, `cp`, `mv`, `touch`) require
//! root (ring 1). The default first-boot login is root, so all commands
//! work out of the box. For regular users, these commands print
//! "Permission denied".

use crate::features;
use crate::io;
use crate::path;
use crate::pipeline;
use crate::syscalls;

/// Maximum number of arguments per command line.
pub const MAX_ARGS: usize = 16;

/// Maximum input line length (matches main.rs).
const LINE_MAX: usize = 256;

/// Dispatch a tokenized command line.
pub fn dispatch(args: &[&[u8]]) {
    // Reap finished background jobs before running any command.
    unsafe {
        features::job_reap();
    }
    if args.is_empty() {
        return;
    }
    let cmd = args[0];
    let rest = &args[1..];

    if cmd == b"help" || cmd == b"?" {
        cmd_help(rest);
    } else if cmd == b"echo" {
        cmd_echo(rest);
    } else if cmd == b"export" {
        cmd_export(rest);
    } else if cmd == b"set" {
        cmd_set(rest);
    } else if cmd == b"unset" {
        cmd_unset(rest);
    } else if cmd == b"pwd" {
        cmd_pwd(rest);
    } else if cmd == b"cd" {
        cmd_cd(rest);
    } else if cmd == b"ls" {
        cmd_ls(rest);
    } else if cmd == b"cat" {
        cmd_cat(rest);
    } else if cmd == b"rm" {
        cmd_rm(rest);
    } else if cmd == b"mkdir" {
        cmd_mkdir(rest);
    } else if cmd == b"cp" {
        cmd_cp(rest);
    } else if cmd == b"mv" {
        cmd_mv(rest);
    } else if cmd == b"touch" {
        cmd_touch(rest);
    } else if cmd == b"stat" {
        cmd_stat(rest);
    } else if cmd == b"whoami" {
        cmd_whoami(rest);
    } else if cmd == b"uname" {
        cmd_uname(rest);
    } else if cmd == b"clear" {
        cmd_clear(rest);
    } else if cmd == b"exit" || cmd == b"logout" {
        cmd_exit(rest);
    } else if cmd == b"jobs" {
        cmd_jobs(rest);
    } else if cmd == b"fg" {
        cmd_fg(rest);
    } else if cmd == b"bg" {
        cmd_bg(rest);
    } else if cmd == b"exec" {
        cmd_exec(rest);
    } else if cmd == b"run" {
        cmd_run(rest);
    } else if cmd == b"source" || cmd == b"." {
        cmd_source(rest);
    } else if cmd == b"date" {
        cmd_date(rest);
    } else if cmd == b"ver" || cmd == b"version" {
        cmd_ver(rest);
    } else {
        io::write_raw(b"osh: ");
        io::write_raw(cmd);
        io::write_raw(b": command not found (try 'help')\n");
    }
}

// ─── help ────────────────────────────────────────────────────────────────

fn cmd_help(_args: &[&[u8]]) {
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

// ─── echo ────────────────────────────────────────────────────────────────

fn cmd_echo(args: &[&[u8]]) {
    for (i, a) in args.iter().enumerate() {
        if i > 0 {
            io::write_byte(b' ');
        }
        io::write_raw(a);
    }
    io::newline();
}

// ─── export ──────────────────────────────────────────────────────────────

fn cmd_export(args: &[&[u8]]) {
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
        // `export VAR` (without =value) is accepted but is a no-op —
        // all our variables are "exported" by default.
    }
}

// ─── set ─────────────────────────────────────────────────────────────────

fn cmd_set(_args: &[&[u8]]) {
    unsafe {
        features::env_list();
    }
}

// ─── unset ───────────────────────────────────────────────────────────────

fn cmd_unset(args: &[&[u8]]) {
    for a in args {
        unsafe {
            features::env_unset(a);
        }
    }
}

// ─── pwd ─────────────────────────────────────────────────────────────────

fn cmd_pwd(_args: &[&[u8]]) {
    let mut buf = [0u8; path::PATH_MAX];
    let n = unsafe { syscalls::getcwd(buf.as_mut_ptr(), path::PATH_MAX as u64) };
    if n > 0 {
        io::write_raw(&buf[..n as usize]);
    } else {
        io::write_raw(b"/");
    }
    io::newline();
}

// ─── cd ──────────────────────────────────────────────────────────────────

fn cmd_cd(args: &[&[u8]]) {
    let target: &[u8] = if args.is_empty() { b"/" } else { args[0] };

    let mut abs = [0u8; path::PATH_MAX];
    let len = path::resolve(target, &mut abs);
    if len == 0 {
        io::write_error("cd: path too long");
        return;
    }

    let ret = unsafe { syscalls::chdir(abs.as_ptr()) };
    if ret < 0 {
        io::write_error_errno("cd", ret);
    }
    // On success, the kernel's cwd is updated. No need to print anything.
}

// ─── ls ──────────────────────────────────────────────────────────────────

fn cmd_ls(args: &[&[u8]]) {
    // Parse arguments: first non-flag is the path, -l enables long format.
    let mut path_arg: &[u8] = b"";
    let mut long_format = false;
    for a in args {
        if a == b"-l" {
            long_format = true;
        } else if a == b"-a" {
            // -a is accepted but OnyxFS readdir returns all entries anyway.
            // We don't filter "." and ".." out, so this is a no-op.
        } else if !a.is_empty() && a[0] == b'-' && a.len() > 1 {
            // Unknown flag — ignore.
        } else if path_arg.is_empty() {
            path_arg = a;
        }
    }

    let mut abs = [0u8; path::PATH_MAX];
    let target = if path_arg.is_empty() { b"." } else { path_arg };
    let len = path::resolve(target, &mut abs);
    if len == 0 {
        io::write_error("ls: path too long");
        return;
    }

    if long_format {
        ls_long(&abs[..len]);
    } else {
        ls_short(&abs[..len]);
    }
}

/// Short `ls` — just print entry names, one per line.
fn ls_short(dir_path: &[u8]) {
    let mut path_buf = [0u8; path::PATH_MAX];
    path_buf[..dir_path.len()].copy_from_slice(dir_path);
    path_buf[dir_path.len()] = 0;

    let mut name = [0u8; 256];
    let mut any = false;
    loop {
        let ret =
            unsafe { syscalls::readdir(path_buf.as_ptr(), name.as_mut_ptr(), name.len() as u64) };
        if ret <= 0 {
            if ret < 0 && !any {
                io::write_error_errno("ls", ret);
            }
            break;
        }
        any = true;
        // Find NUL terminator.
        let mut nlen = 0;
        while nlen < name.len() && name[nlen] != 0 {
            nlen += 1;
        }
        io::write_raw(&name[..nlen]);
        io::newline();
    }
}

/// Long `ls -l` — print type, size, and name for each entry.
fn ls_long(dir_path: &[u8]) {
    let mut path_buf = [0u8; path::PATH_MAX];
    path_buf[..dir_path.len()].copy_from_slice(dir_path);
    path_buf[dir_path.len()] = 0;

    let mut name = [0u8; 256];
    let mut any = false;
    loop {
        let ret =
            unsafe { syscalls::readdir(path_buf.as_ptr(), name.as_mut_ptr(), name.len() as u64) };
        if ret <= 0 {
            if ret < 0 && !any {
                io::write_error_errno("ls", ret);
            }
            break;
        }
        any = true;

        let mut nlen = 0;
        while nlen < name.len() && name[nlen] != 0 {
            nlen += 1;
        }
        let entry_name = &name[..nlen];

        // Stat the entry to get type and size.
        let mut full_path = [0u8; path::PATH_MAX];
        let flen = join_path(dir_path, entry_name, &mut full_path);
        if flen == 0 {
            // Path too long — print without stat info.
            io::write_raw(b"????????  ??????  ");
            io::write_raw(entry_name);
            io::newline();
            continue;
        }

        let mut st = [0u8; 256];
        let sret = unsafe { syscalls::stat(full_path.as_ptr(), st.as_mut_ptr()) };
        if sret < 0 {
            io::write_raw(b"????????  ??????  ");
            io::write_raw(entry_name);
            io::newline();
            continue;
        }

        // struct stat layout (see kernel UserStat):
        //   st_mode: u32 at offset 16
        //   st_size: i64 at offset 48
        let st_mode = u32::from_le_bytes([st[16], st[17], st[18], st[19]]);
        let st_size = i64::from_le_bytes([
            st[48], st[49], st[50], st[51], st[52], st[53], st[54], st[55],
        ]);

        // Determine type from st_mode's S_IFMT bits.
        let ifmt = st_mode & 0o170_000;
        let type_ch: u8 = if ifmt == 0o040_000 {
            b'd'
        } else if ifmt == 0o100_000 {
            b'-'
        } else if ifmt == 0o120_000 {
            b'c'
        } else {
            b'?'
        };

        io::write_byte(type_ch);
        io::write_raw(b"rwxr-xr-x  ");
        io::write_u64_field(st_size as u64, 8);
        io::write_raw(b"  ");
        io::write_raw(entry_name);
        io::newline();
    }
}

/// Join a directory path and a name into `out`, NUL-terminated.
fn join_path(dir: &[u8], name: &[u8], out: &mut [u8; path::PATH_MAX]) -> usize {
    if dir.len() >= path::PATH_MAX {
        return 0;
    }
    out[..dir.len()].copy_from_slice(dir);
    let mut olen = dir.len();
    // Add separator if dir doesn't end with '/'.
    if olen > 0 && out[olen - 1] != b'/' {
        if olen >= path::PATH_MAX - 1 {
            return 0;
        }
        out[olen] = b'/';
        olen += 1;
    }
    if olen + name.len() >= path::PATH_MAX {
        return 0;
    }
    out[olen..olen + name.len()].copy_from_slice(name);
    olen += name.len();
    out[olen] = 0;
    olen
}

// ─── cat ─────────────────────────────────────────────────────────────────

fn cmd_cat(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("cat: missing file operand (try 'help')");
        return;
    }

    for a in args {
        let mut abs = [0u8; path::PATH_MAX];
        let len = path::resolve(a, &mut abs);
        if len == 0 {
            io::write_error("cat: path too long");
            continue;
        }

        let fd = unsafe { syscalls::open(abs.as_ptr(), syscalls::O_RDONLY as u64, 0) };
        if fd < 0 {
            io::write_error_errno("cat", fd);
            continue;
        }

        let mut buf = [0u8; 512];
        loop {
            let n = unsafe { syscalls::read_fd(fd as u64, buf.as_mut_ptr(), buf.len() as u64) };
            if n <= 0 {
                break;
            }
            io::write_raw(&buf[..n as usize]);
        }
        let _ = unsafe {
            syscalls::close(fd as u64);
        };
    }
}

// ─── rm ──────────────────────────────────────────────────────────────────

fn cmd_rm(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("rm: missing operand (try 'help')");
        return;
    }

    for a in args {
        let mut abs = [0u8; path::PATH_MAX];
        let len = path::resolve(a, &mut abs);
        if len == 0 {
            io::write_error("rm: path too long");
            continue;
        }

        let ret = unsafe { syscalls::unlink(abs.as_ptr()) };
        if ret < 0 {
            io::write_error_errno("rm", ret);
        }
    }
}

// ─── mkdir ───────────────────────────────────────────────────────────────

fn cmd_mkdir(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("mkdir: missing operand (try 'help')");
        return;
    }

    for a in args {
        let mut abs = [0u8; path::PATH_MAX];
        let len = path::resolve(a, &mut abs);
        if len == 0 {
            io::write_error("mkdir: path too long");
            continue;
        }

        let ret = unsafe { syscalls::mkdir(abs.as_ptr()) };
        if ret < 0 {
            // EEXIST is common when the directory already exists —
            // print a softer message but still report it.
            if ret == syscalls::EEXIST {
                io::write_raw(b"osh: mkdir: ");
                io::write_raw(a);
                io::write_raw(b": directory exists\n");
            } else {
                io::write_error_errno("mkdir", ret);
            }
        }
    }
}

// ─── cp ──────────────────────────────────────────────────────────────────

fn cmd_cp(args: &[&[u8]]) {
    if args.len() < 2 {
        io::write_error("cp: missing operand (usage: cp <src> <dst>)");
        return;
    }

    let src_in = args[0];
    let dst_in = args[1];

    let mut src_abs = [0u8; path::PATH_MAX];
    let mut dst_abs = [0u8; path::PATH_MAX];
    let slen = path::resolve(src_in, &mut src_abs);
    let dlen = path::resolve(dst_in, &mut dst_abs);
    if slen == 0 || dlen == 0 {
        io::write_error("cp: path too long");
        return;
    }

    // Open source for reading.
    let src_fd = unsafe { syscalls::open(src_abs.as_ptr(), syscalls::O_RDONLY as u64, 0) };
    if src_fd < 0 {
        io::write_error_errno("cp", src_fd);
        return;
    }

    // Create destination file using SYS_create (root-only).
    // create() returns a writable fd token directly — no need to re-open.
    let dst_fd = unsafe { syscalls::create(dst_abs.as_ptr(), 0, 0) };
    if dst_fd < 0 {
        io::write_error_errno("cp: cannot create destination", dst_fd);
        unsafe {
            syscalls::close(src_fd as u64);
        }
        return;
    }

    // Copy data from source to destination.
    copy_loop(src_fd as u64, dst_fd as u64);

    unsafe {
        syscalls::close(dst_fd as u64);
        syscalls::close(src_fd as u64);
    }
}

/// Copy data from `src_fd` to `dst_fd` in 512-byte chunks.
fn copy_loop(src_fd: u64, dst_fd: u64) {
    let mut buf = [0u8; 512];
    loop {
        let n = unsafe { syscalls::read_fd(src_fd, buf.as_mut_ptr(), buf.len() as u64) };
        if n <= 0 {
            break;
        }
        let n = n as usize;
        let mut written = 0usize;
        while written < n {
            let w = unsafe { syscalls::write_fd(dst_fd, buf[written..].as_ptr(), n - written) };
            if w <= 0 {
                io::write_error("cp: write error");
                return;
            }
            written += w as usize;
        }
    }
}

// ─── mv ──────────────────────────────────────────────────────────────────

fn cmd_mv(args: &[&[u8]]) {
    if args.len() < 2 {
        io::write_error("mv: missing operand (usage: mv <src> <dst>)");
        return;
    }

    let src_in = args[0];
    let dst_in = args[1];

    let mut src_abs = [0u8; path::PATH_MAX];
    let mut dst_abs = [0u8; path::PATH_MAX];
    let slen = path::resolve(src_in, &mut src_abs);
    let dlen = path::resolve(dst_in, &mut dst_abs);
    if slen == 0 || dlen == 0 {
        io::write_error("mv: path too long");
        return;
    }

    // Try rename() first — it's atomic and fast.
    let ret = unsafe { syscalls::rename(src_abs.as_ptr(), dst_abs.as_ptr()) };
    if ret == 0 {
        return; // Success.
    }

    // If rename failed, fall back to cp + rm.
    // (Common case: cross-directory rename may not be supported by OnyxFS.)
    io::write_raw(b"osh: mv: rename failed, falling back to copy+remove\n");
    cmd_cp(args);
    // Only remove source if the copy succeeded (cp prints its own errors).
    let rm_ret = unsafe { syscalls::unlink(src_abs.as_ptr()) };
    if rm_ret < 0 {
        io::write_error_errno("mv: cannot remove source", rm_ret);
    }
}

// ─── touch ───────────────────────────────────────────────────────────────

fn cmd_touch(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("touch: missing operand (try 'help')");
        return;
    }

    for a in args {
        let mut abs = [0u8; path::PATH_MAX];
        let len = path::resolve(a, &mut abs);
        if len == 0 {
            io::write_error("touch: path too long");
            continue;
        }

        // Use SYS_create directly. If the file already exists, create
        // returns EEXIST and we just close the existing fd (no error).
        let ret = unsafe { syscalls::create(abs.as_ptr(), 0, 0) };
        if ret >= 0 {
            unsafe {
                syscalls::close(ret as u64);
            }
        } else if ret != syscalls::EEXIST {
            io::write_error_errno("touch", ret);
        }
        // If ret == EEXIST, the file already exists — touch succeeds silently.
    }
}

// ─── stat ────────────────────────────────────────────────────────────────

fn cmd_stat(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("stat: missing operand (try 'help')");
        return;
    }

    let mut abs = [0u8; path::PATH_MAX];
    let len = path::resolve(args[0], &mut abs);
    if len == 0 {
        io::write_error("stat: path too long");
        return;
    }

    let mut st = [0u8; 256];
    let ret = unsafe { syscalls::stat(abs.as_ptr(), st.as_mut_ptr()) };
    if ret < 0 {
        io::write_error_errno("stat", ret);
        return;
    }

    // Parse struct stat fields (see kernel UserStat in fs_sys/open_close.rs).
    let st_dev = u64::from_le_bytes([st[0], st[1], st[2], st[3], st[4], st[5], st[6], st[7]]);
    let st_ino = u64::from_le_bytes([st[8], st[9], st[10], st[11], st[12], st[13], st[14], st[15]]);
    let st_mode = u32::from_le_bytes([st[16], st[17], st[18], st[19]]);
    let st_nlink = u32::from_le_bytes([st[20], st[21], st[22], st[23]]);
    let st_uid = u32::from_le_bytes([st[24], st[25], st[26], st[27]]);
    let st_gid = u32::from_le_bytes([st[28], st[29], st[30], st[31]]);
    // st_rdev at offset 40 (after 4 bytes padding at 36)
    let st_rdev = u64::from_le_bytes([
        st[40], st[41], st[42], st[43], st[44], st[45], st[46], st[47],
    ]);
    let st_size = i64::from_le_bytes([
        st[48], st[49], st[50], st[51], st[52], st[53], st[54], st[55],
    ]);
    let st_blksize = i64::from_le_bytes([
        st[56], st[57], st[58], st[59], st[60], st[61], st[62], st[63],
    ]);
    let st_blocks = i64::from_le_bytes([
        st[64], st[65], st[66], st[67], st[68], st[69], st[70], st[71],
    ]);
    let st_mtime = i64::from_le_bytes([
        st[88], st[89], st[90], st[91], st[92], st[93], st[94], st[95],
    ]);

    // Determine file type from st_mode.
    let ifmt = st_mode & 0o170_000;
    let type_str: &[u8] = if ifmt == 0o040_000 {
        b"directory"
    } else if ifmt == 0o100_000 {
        b"regular file"
    } else if ifmt == 0o120_000 {
        b"character device"
    } else {
        b"unknown"
    };

    // Print each line as a single write to avoid any compiler optimization
    // issues with consecutive write_str calls.
    io::write_raw(b"  File: ");
    io::write_raw(&abs[..len]);
    io::newline();

    io::write_raw(b"  Size: ");
    io::write_i64(st_size);
    io::write_raw(b"       Type: ");
    io::write_raw(type_str);
    io::newline();

    io::write_raw(b"  Inode: ");
    io::write_u64(st_ino);
    io::write_raw(b"       Links: ");
    io::write_u64(st_nlink as u64);
    io::newline();

    io::write_raw(b"  Device: ");
    io::write_hex(st_dev);
    io::write_raw(b"   Rdev: ");
    io::write_hex(st_rdev);
    io::newline();

    io::write_raw(b"  Mode: ");
    io::write_hex(st_mode as u64);
    io::write_raw(b"   Uid: ");
    io::write_u64(st_uid as u64);
    io::write_raw(b"   Gid: ");
    io::write_u64(st_gid as u64);
    io::newline();

    io::write_raw(b"  Blksize: ");
    io::write_i64(st_blksize);
    io::write_raw(b"   Blocks: ");
    io::write_i64(st_blocks);
    io::newline();

    io::write_raw(b"  Mtime: ");
    io::write_u64(st_mtime as u64);
    io::write_raw(b" (epoch seconds)");
    io::newline();
}

// ─── whoami ──────────────────────────────────────────────────────────────

fn cmd_whoami(_args: &[&[u8]]) {
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

// ─── uname ───────────────────────────────────────────────────────────────

fn cmd_uname(_args: &[&[u8]]) {
    let mut buf = [0u8; 390];
    let ret = unsafe { syscalls::uname(buf.as_mut_ptr()) };
    if ret < 0 {
        io::write_error_errno("uname", ret);
        return;
    }

    // uname buffer: 5 fields, each 65 bytes (NUL-terminated).
    // Field offsets: 0, 65, 130, 195, 260
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

// ─── date ────────────────────────────────────────────────────────────────

fn cmd_date(_args: &[&[u8]]) {
    let mut ts = [0u64; 2]; // [seconds, nanoseconds]
    let ret = unsafe { syscalls::clock_gettime(0, ts.as_mut_ptr()) }; // CLOCK_REALTIME = 0
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

// ─── clear ───────────────────────────────────────────────────────────────

fn cmd_clear(_args: &[&[u8]]) {
    // ANSI clear screen + move cursor to (1,1).
    io::write_raw(b"\x1b[2J\x1b[H");
}

// ─── exit ────────────────────────────────────────────────────────────────

fn cmd_exit(_args: &[&[u8]]) {
    io::write_line("logout");
    unsafe {
        syscalls::exit(0);
    }
}

// ─── ver ─────────────────────────────────────────────────────────────────

fn cmd_ver(_args: &[&[u8]]) {
    io::write_line("OnyxShell v0.2.0 — built-in command shell for OnyxOS");
    io::write_line("Copyright (c) 2024-2025 loki5512344");
    io::write_line("License: GPL-3.0-or-later");
}

// ── PATH search helper ──────────────────────────────────────────────────

/// Search `$PATH` for a command that has no `/` in its name.
/// Returns an absolute NUL-terminated path if found.
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

// ─── exec ────────────────────────────────────────────────────────────────

/// exec <path> [args] — replace the current shell process with a binary.
fn cmd_exec(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("exec: missing path (usage: exec <path> [args...])");
        return;
    }

    let mut path_buf = [0u8; path::PATH_MAX];
    let len = if args[0].contains(&b'/') {
        path::resolve(args[0], &mut path_buf)
    } else if let Some(found) = search_path(args[0]) {
        let flen = found.iter().position(|&b| b == 0).unwrap_or(path::PATH_MAX);
        path_buf[..flen].copy_from_slice(&found[..flen]);
        flen
    } else {
        path::resolve(args[0], &mut path_buf)
    };
    if len == 0 {
        io::write_error("exec: path too long");
        return;
    }

    // ── Shebang check ──
    if let Some(interp_buf) = check_shebang(path_buf.as_ptr()) {
        let interp_len = interp_buf.iter().position(|&b| b == 0).unwrap_or(256);
        if interp_len > 0 && &interp_buf[..interp_len] == b"/bin/osh" {
            // Run as osh script in current context, then exit
            do_script(&path_buf[..len]);
            unsafe {
                syscalls::exit(0);
            }
        }
        // Exec with interpreter: argv = [interp, script, args[1..]]
        let mut argv_strs: [[u8; path::PATH_MAX]; MAX_ARGS] = [[0; path::PATH_MAX]; MAX_ARGS];
        let mut argv_ptrs = [0u64; MAX_ARGS + 1];
        let mut argc = 0usize;
        argv_strs[argc][..interp_len].copy_from_slice(&interp_buf[..interp_len]);
        argv_strs[argc][interp_len] = 0;
        argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
        argc += 1;
        let path_slice = &path_buf[..len];
        argv_strs[argc][..path_slice.len()].copy_from_slice(path_slice);
        argv_strs[argc][path_slice.len()] = 0;
        argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
        argc += 1;
        for i in 1..args.len().min(MAX_ARGS - 2) {
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
        let mut envp_strs =
            [[0u8; features::ENV_KEY_MAX + features::ENV_VAL_MAX + 2]; features::ENV_MAX];
        let mut envp_ptrs = [0u64; features::ENV_MAX + 1];
        let _n_env = unsafe { features::build_envp(&mut envp_strs, &mut envp_ptrs) };
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

    // Build argv: array of pointers to NUL-terminated strings.
    let mut argv_strs: [[u8; path::PATH_MAX]; MAX_ARGS] = [[0; path::PATH_MAX]; MAX_ARGS];
    let mut argv_ptrs = [0u64; MAX_ARGS + 1];
    let argc = args.len().min(MAX_ARGS);
    for i in 0..argc {
        let arg = args[i];
        if arg.len() >= path::PATH_MAX {
            io::write_error("exec: argument too long");
            return;
        }
        argv_strs[i][..arg.len()].copy_from_slice(arg);
        argv_strs[i][arg.len()] = 0;
        argv_ptrs[i] = argv_strs[i].as_ptr() as u64;
    }
    argv_ptrs[argc] = 0;

    // Build envp from the current environment.
    let mut envp_strs =
        [[0u8; features::ENV_KEY_MAX + features::ENV_VAL_MAX + 2]; features::ENV_MAX];
    let mut envp_ptrs = [0u64; features::ENV_MAX + 1];
    let _n_env = unsafe { features::build_envp(&mut envp_strs, &mut envp_ptrs) };

    let ret =
        unsafe { syscalls::execve(path_buf.as_ptr(), argv_ptrs.as_ptr(), envp_ptrs.as_ptr()) };
    io::write_error_errno("exec", ret);
}

// ─── run ─────────────────────────────────────────────────────────────────

/// run <path> [args] — spawn a binary as a child and wait for it.
/// Root-only (SYS_spawn requires ring ≤ 1).
fn cmd_run(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("run: missing path (usage: run <path> [args...])");
        return;
    }

    let mut path_buf = [0u8; path::PATH_MAX];
    let len = if args[0].contains(&b'/') {
        path::resolve(args[0], &mut path_buf)
    } else if let Some(found) = search_path(args[0]) {
        let flen = found.iter().position(|&b| b == 0).unwrap_or(path::PATH_MAX);
        path_buf[..flen].copy_from_slice(&found[..flen]);
        flen
    } else {
        path::resolve(args[0], &mut path_buf)
    };
    if len == 0 {
        io::write_error("run: path too long");
        return;
    }

    // ── Shebang check ──
    if let Some(interp_buf) = check_shebang(path_buf.as_ptr()) {
        let interp_len = interp_buf.iter().position(|&b| b == 0).unwrap_or(256);
        let target: &[u8] = if interp_len > 0 && &interp_buf[..interp_len] == b"/bin/osh" {
            b"/bin/osh"
        } else {
            &interp_buf[..interp_len]
        };
        let mut argv_strs: [[u8; path::PATH_MAX]; MAX_ARGS] = [[0; path::PATH_MAX]; MAX_ARGS];
        let mut argv_ptrs = [0u64; MAX_ARGS + 1];
        let mut argc = 0usize;
        // argv[0] = interpreter
        argv_strs[argc][..target.len()].copy_from_slice(target);
        argv_strs[argc][target.len()] = 0;
        argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
        argc += 1;
        // argv[1] = script path
        let path_slice = &path_buf[..len];
        argv_strs[argc][..path_slice.len()].copy_from_slice(path_slice);
        argv_strs[argc][path_slice.len()] = 0;
        argv_ptrs[argc] = argv_strs[argc].as_ptr() as u64;
        argc += 1;
        // argv[2..] = remaining args
        for i in 1..args.len().min(MAX_ARGS - 2) {
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

    // Build argv.
    let mut argv_strs: [[u8; path::PATH_MAX]; MAX_ARGS] = [[0; path::PATH_MAX]; MAX_ARGS];
    let mut argv_ptrs = [0u64; MAX_ARGS + 1];
    let argc = args.len().min(MAX_ARGS);
    for i in 0..argc {
        let arg = args[i];
        if arg.len() >= path::PATH_MAX {
            io::write_error("run: argument too long");
            return;
        }
        argv_strs[i][..arg.len()].copy_from_slice(arg);
        argv_strs[i][arg.len()] = 0;
        argv_ptrs[i] = argv_strs[i].as_ptr() as u64;
    }
    argv_ptrs[argc] = 0;

    // Spawn the child. ring_hint=0 means "inherit caller's ring".
    let pid = unsafe { syscalls::spawn(path_buf.as_ptr(), argv_ptrs.as_ptr(), 0) };
    if pid < 0 {
        io::write_error_errno("run", pid);
        return;
    }

    // Wait for the child to exit.
    let mut status: i32 = 0;
    let waited = unsafe { syscalls::wait(&mut status) };
    if waited < 0 {
        io::write_error_errno("run: wait", waited);
        return;
    }

    // Print exit status if non-zero.
    if status != 0 {
        io::write_raw(b"osh: process exited with code ");
        io::write_i64(status as i64);
        io::newline();
    }
}

// ─── jobs ────────────────────────────────────────────────────────────────

fn cmd_jobs(_args: &[&[u8]]) {
    unsafe {
        features::job_list();
    }
}

// ─── fg ──────────────────────────────────────────────────────────────────

fn cmd_fg(args: &[&[u8]]) {
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

// ─── bg ──────────────────────────────────────────────────────────────────

fn cmd_bg(args: &[&[u8]]) {
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

/// Parse a job specifier like `%1` → 1. Returns 0 on invalid input.
fn parse_job_id(arg: &[u8]) -> usize {
    if arg.len() > 1 && arg[0] == b'%' {
        let mut id = 0usize;
        for &b in &arg[1..] {
            if b >= b'0' && b <= b'9' {
                id = id * 10 + (b - b'0') as usize;
            } else {
                return 0;
            }
        }
        id
    } else {
        0
    }
}

// ─── source ──────────────────────────────────────────────────────────────

fn cmd_source(args: &[&[u8]]) {
    if args.is_empty() {
        io::write_error("source: missing file operand (try 'help')");
        return;
    }
    do_script(args[0]);
}

/// Execute a script file: open, read lines, dispatch each.
/// Used by `source`, batch mode, and `#!/bin/osh` shebang handling.
pub fn do_script(input: &[u8]) {
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

    let mut line_buf = [0u8; LINE_MAX];
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
        } else if c[0] != b'\r' && line_pos < LINE_MAX - 1 {
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

/// Execute a single line: tilde expansion, variable expansion,
/// pipe/redirect check, tokenization, glob expansion, and dispatch.
fn execute_line(line: &[u8]) {
    let expanded = unsafe { features::expand_tilde(line) };
    let expanded = unsafe { features::expand_vars(expanded.as_slice()) };
    let expanded_slice = expanded.as_slice();

    let has_pipe_or_redirect = expanded_slice
        .iter()
        .any(|&b| b == b'|' || b == b'>' || b == b'<');
    if has_pipe_or_redirect {
        static mut G_EXPANDED: [u8; LINE_MAX] = [0u8; LINE_MAX];
        let n = expanded_slice.len().min(LINE_MAX - 1);
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

    dispatch(unsafe { &G_ARGS[..n_args] });
}

// ── Shebang check ──────────────────────────────────────────────────────

/// Read the first line of a binary and check for `#!` shebang.
/// Returns the interpreter path (NUL-terminated), or None.
fn check_shebang(path: *const u8) -> Option<[u8; 256]> {
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

    // Skip "#!" and whitespace to find interpreter path.
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
