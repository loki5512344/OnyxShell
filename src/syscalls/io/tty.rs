pub use super::super::consts::*;
use core::arch::asm;

pub unsafe fn write(fd: u64, buf: *const u8, len: usize) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_write,
        in("a0") fd,
        in("a1") buf as usize,
        in("a2") len,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn read(fd: u64, buf: *mut u8, len: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_read,
        in("a0") fd,
        in("a1") buf as usize,
        in("a2") len as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn ioctl(fd: u64, request: u64, arg: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_ioctl,
        in("a0") fd,
        in("a1") request,
        in("a2") arg,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn enable_raw_mode() -> i64 {
    ioctl(0, TIOCSRAW, 0)
}

pub unsafe fn disable_raw_mode() -> i64 {
    ioctl(0, TIOCRRAW, 0)
}

pub unsafe fn uname(buf: *mut u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_uname,
        in("a0") buf as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn clock_gettime(clk_id: u64, ts: *mut u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_clock_gettime,
        in("a0") clk_id,
        in("a1") ts as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn isatty(fd: u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_isatty,
        in("a0") fd,
        lateout("a0") ret,
    );
    ret
}
