use crate::io;
use crate::path;
use crate::syscalls;
pub(crate) fn cmd_cat(args: &[&[u8]]) {
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
pub(crate) fn cmd_rm(args: &[&[u8]]) {
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
pub(crate) fn cmd_mkdir(args: &[&[u8]]) {
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
pub(crate) fn cmd_touch(args: &[&[u8]]) {
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

        let ret = unsafe { syscalls::create(abs.as_ptr(), 0, 0) };
        if ret >= 0 {
            unsafe {
                syscalls::close(ret as u64);
            }
        } else if ret != syscalls::EEXIST {
            io::write_error_errno("touch", ret);
        }
    }
}
pub(crate) fn cmd_stat(args: &[&[u8]]) {
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

    let st_dev = u64::from_le_bytes([st[0], st[1], st[2], st[3], st[4], st[5], st[6], st[7]]);
    let st_ino = u64::from_le_bytes([st[8], st[9], st[10], st[11], st[12], st[13], st[14], st[15]]);
    let st_mode = u32::from_le_bytes([st[16], st[17], st[18], st[19]]);
    let st_nlink = u32::from_le_bytes([st[20], st[21], st[22], st[23]]);
    let st_uid = u32::from_le_bytes([st[24], st[25], st[26], st[27]]);
    let st_gid = u32::from_le_bytes([st[28], st[29], st[30], st[31]]);
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
