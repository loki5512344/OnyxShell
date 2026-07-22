pub use super::consts::*;
use core::arch::asm;

pub unsafe fn open(path: *const u8, flags: u64, mode: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_open,
        in("a0") path as usize,
        in("a1") flags,
        in("a2") mode,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn close(fd: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_close,
        in("a0") fd,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn read_fd(fd: u64, buf: *mut u8, len: u64) -> i64 {
    super::io::tty::read(fd, buf, len)
}

pub unsafe fn write_fd(fd: u64, buf: *const u8, len: usize) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_write_fd,
        in("a0") fd,
        in("a1") buf as usize,
        in("a2") len,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn lseek(fd: u64, offset: i64, whence: u32) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_lseek,
        in("a0") fd,
        in("a1") offset,
        in("a2") whence as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn stat(path: *const u8, st_buf: *mut u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_stat,
        in("a0") path as usize,
        in("a1") st_buf as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn fstat(fd: u64, st_buf: *mut u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_fstat,
        in("a0") fd,
        in("a1") st_buf as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn readdir(dir: *const u8, name_out: *mut u8, len: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_readdir,
        in("a0") dir as usize,
        in("a1") name_out as usize,
        in("a2") len as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn create(path: *const u8, mode: u64, reserved: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_create,
        in("a0") path as usize,
        in("a1") mode,
        in("a2") reserved,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn mkdir(path: *const u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_mkdir,
        in("a0") path as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn unlink(path: *const u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_unlink,
        in("a0") path as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn rename(old_path: *const u8, new_path: *const u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_rename,
        in("a0") old_path as usize,
        in("a1") new_path as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn chdir(path: *const u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_chdir,
        in("a0") path as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn getcwd(buf: *mut u8, len: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getcwd,
        in("a0") buf as usize,
        in("a1") len,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn getdents64(fd: u64, buf: *mut u8, count: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getdents64,
        in("a0") fd,
        in("a1") buf as usize,
        in("a2") count,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn pipe(out_fds: *mut u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_pipe,
        in("a0") out_fds as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn dup(fd: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_dup,
        in("a0") fd,
        lateout("a0") ret,
    );
    ret
}
