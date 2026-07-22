use crate::io;
use crate::path;
use crate::syscalls;

pub(crate) fn cmd_pwd(_args: &[&[u8]]) {
    let mut buf = [0u8; path::PATH_MAX];
    let n = unsafe { syscalls::getcwd(buf.as_mut_ptr(), path::PATH_MAX as u64) };
    if n > 0 {
        io::write_raw(&buf[..n as usize]);
    } else {
        io::write_raw(b"/");
    }
    io::newline();
}

pub(crate) fn cmd_cd(args: &[&[u8]]) {
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
}

pub(crate) fn cmd_ls(args: &[&[u8]]) {
    let mut path_arg: &[u8] = b"";
    let mut long_format = false;
    for a in args {
        if a == b"-l" {
            long_format = true;
        } else if a == b"-a" {
        } else if !a.is_empty() && a[0] == b'-' && a.len() > 1 {
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
        let mut nlen = 0;
        while nlen < name.len() && name[nlen] != 0 {
            nlen += 1;
        }
        io::write_raw(&name[..nlen]);
        io::newline();
    }
}

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

        let mut full_path = [0u8; path::PATH_MAX];
        let flen = join_path(dir_path, entry_name, &mut full_path);
        if flen == 0 {
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

        let st_mode = u32::from_le_bytes([st[16], st[17], st[18], st[19]]);
        let st_size = i64::from_le_bytes([
            st[48], st[49], st[50], st[51], st[52], st[53], st[54], st[55],
        ]);

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

fn join_path(dir: &[u8], name: &[u8], out: &mut [u8; path::PATH_MAX]) -> usize {
    if dir.len() >= path::PATH_MAX {
        return 0;
    }
    out[..dir.len()].copy_from_slice(dir);
    let mut olen = dir.len();
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
