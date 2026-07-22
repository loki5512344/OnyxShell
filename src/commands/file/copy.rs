use crate::io;
use crate::path;
use crate::syscalls;

pub(crate) fn cmd_cp(args: &[&[u8]]) -> bool {
    if args.len() < 2 {
        io::write_error("cp: missing operand (usage: cp <src> <dst>)");
        return false;
    }

    let src_in = args[0];
    let dst_in = args[1];

    let mut src_abs = [0u8; path::PATH_MAX];
    let mut dst_abs = [0u8; path::PATH_MAX];
    let slen = path::resolve(src_in, &mut src_abs);
    let dlen = path::resolve(dst_in, &mut dst_abs);
    if slen == 0 || dlen == 0 {
        io::write_error("cp: path too long");
        return false;
    }

    let src_fd = unsafe { syscalls::open(src_abs.as_ptr(), syscalls::O_RDONLY as u64, 0) };
    if src_fd < 0 {
        io::write_error_errno("cp", src_fd);
        return false;
    }

    let dst_fd = unsafe { syscalls::create(dst_abs.as_ptr(), 0, 0) };
    if dst_fd < 0 {
        io::write_error_errno("cp: cannot create destination", dst_fd);
        unsafe {
            syscalls::close(src_fd as u64);
        }
        return false;
    }

    let copy_ok = copy_loop(src_fd as u64, dst_fd as u64);

    unsafe {
        syscalls::close(dst_fd as u64);
        syscalls::close(src_fd as u64);
    }
    copy_ok
}

fn copy_loop(src_fd: u64, dst_fd: u64) -> bool {
    let mut buf = [0u8; 512];
    loop {
        let n = unsafe { syscalls::read_fd(src_fd, buf.as_mut_ptr(), buf.len() as u64) };
        if n < 0 {
            io::write_error("cp: read error");
            return false;
        }
        if n == 0 {
            return true;
        }
        let n = n as usize;
        let mut written = 0usize;
        while written < n {
            let w = unsafe { syscalls::write_fd(dst_fd, buf[written..].as_ptr(), n - written) };
            if w <= 0 {
                io::write_error("cp: write error");
                return false;
            }
            written += w as usize;
        }
    }
}

pub(crate) fn cmd_mv(args: &[&[u8]]) {
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

    let ret = unsafe { syscalls::rename(src_abs.as_ptr(), dst_abs.as_ptr()) };
    if ret == 0 {
        return;
    }

    io::write_raw(b"osh: mv: rename failed, falling back to copy+remove\n");
    if cmd_cp(args) {
        let rm_ret = unsafe { syscalls::unlink(src_abs.as_ptr()) };
        if rm_ret < 0 {
            io::write_error_errno("mv: cannot remove source", rm_ret);
        }
    } else {
        io::write_error("mv: copy failed, source preserved");
    }
}
