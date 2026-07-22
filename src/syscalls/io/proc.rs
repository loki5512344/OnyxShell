pub use super::super::consts::*;
use core::arch::asm;

pub unsafe fn exit(code: u64) -> ! {
    asm!(
        "ecall",
        in("a7") SYS_exit,
        in("a0") code,
    );
    loop {
        asm!("wfi");
    }
}

pub unsafe fn yield_cpu() {
    let _ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_yield,
        lateout("a0") _ret,
    );
}

pub unsafe fn getpid() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getpid,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn getring() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getring,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn getuid() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getuid,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn fork() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_fork,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn kill(pid: i32, sig: i32) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_kill,
        in("a0") pid as usize,
        in("a1") sig as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn exec(path: *const u8, argv: *const u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_exec,
        in("a0") path as usize,
        in("a1") argv as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn execve(path: *const u8, argv: *const u64, envp: *const u64) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_execve,
        in("a0") path as usize,
        in("a1") argv as usize,
        in("a2") envp as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn spawn(path: *const u8, argv: *const u64, ring_hint: u8) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_spawn,
        in("a0") path as usize,
        in("a1") argv as usize,
        in("a2") ring_hint as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn wait(status_out: *mut i32) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_wait,
        in("a0") status_out as usize,
        lateout("a0") ret,
    );
    ret
}

pub unsafe fn waitpid(pid: u64, status_out: *mut i32, options: u32) -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_waitpid,
        in("a0") pid,
        in("a1") status_out as usize,
        in("a2") options as usize,
        lateout("a0") ret,
    );
    ret
}
