//! Syscall wrappers for OnyxShell.
//!
//! These are direct RISC-V `ecall` wrappers matching the OnyxKernel
//! syscall ABI (v0.4, 83 syscalls). Only the syscalls that the shell
//! actually uses are wrapped here — the full ABI is defined in
//! `kernel/src/syscall/abi.rs` in the OnyxKernel source.
//!
//! ## Privilege notes
//!
//! OnyxKernel has a three-ring privilege model:
//!   - Ring 0 = Kernel (S-mode)
//!   - Ring 1 = Root space (U-mode, PID 1 and root login session)
//!   - Ring 2 = User space (U-mode, regular users)
//!
//! File-mutation syscalls (`unlink`, `mkdir`, `create`, `rename`) are
//! **root-only** (ring ≤ 1). The default first-boot login is root
//! (ring 1), so all shell commands work out of the box. Regular users
//! (ring 2) will get `EPERM` (-1) from `rm`, `mkdir`, `cp`, `mv`, etc.
//! and the shell prints a clear error message.

#![allow(dead_code, non_upper_case_globals)]
use core::arch::asm;

// ── Syscall numbers (from kernel/src/syscall/abi.rs) ─────────────────────
pub const SYS_write: u64     = 1;
pub const SYS_read: u64      = 2;
pub const SYS_exit: u64      = 3;
pub const SYS_yield: u64     = 4;
pub const SYS_getpid: u64    = 5;
pub const SYS_sbrk: u64      = 13;
pub const SYS_open: u64      = 8;
pub const SYS_close: u64     = 9;
pub const SYS_lseek: u64     = 10;
pub const SYS_stat: u64      = 11;
pub const SYS_exec: u64      = 12;
pub const SYS_spawn: u64     = 14;
pub const SYS_wait: u64      = 15;
pub const SYS_readdir: u64   = 16;
pub const SYS_getring: u64   = 17;
pub const SYS_dropring: u64  = 18;
pub const SYS_write_fd: u64  = 24;
pub const SYS_create: u64    = 25;
pub const SYS_mkdir: u64     = 26;
pub const SYS_unlink: u64    = 37;
pub const SYS_rename: u64    = 38;
pub const SYS_chdir: u64     = 39;
pub const SYS_getcwd: u64    = 40;
pub const SYS_getuid: u64    = 45;
pub const SYS_uname: u64     = 48;
pub const SYS_fstat: u64     = 50;
pub const SYS_waitpid: u64   = 51;
pub const SYS_getdents64: u64 = 52;
pub const SYS_execve: u64    = 58;
pub const SYS_clock_gettime: u64 = 64;
pub const SYS_isatty: u64    = 66;
pub const SYS_pipe: u64      = 36;
pub const SYS_dup: u64       = 35;
pub const SYS_fork: u64      = 63;
pub const SYS_getdents: u64  = 77;
pub const SYS_ioctl: u64     = 53;

// ── ioctl requests (OnyxOS extensions for raw terminal mode) ────────────
pub const TIOCSRAW: u64   = 0x5421;  // enable raw mode for fd 0
pub const TIOCRRAW: u64   = 0x5422;  // disable raw mode
pub const TIOCGRAW: u64   = 0x5423;  // query raw mode (returns 1/0)

// ── open() flags (Linux-compatible) ──────────────────────────────────────
pub const O_RDONLY: u32  = 0;
pub const O_WRONLY: u32  = 1;
pub const O_RDWR: u32    = 2;
pub const O_CREAT: u32   = 1 << 6;   // 0x40
pub const O_TRUNC: u32   = 1 << 9;   // 0x200
pub const O_APPEND: u32  = 1 << 10;  // 0x400

// ── lseek() whence ───────────────────────────────────────────────────────
pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

// ── waitpid() options ────────────────────────────────────────────────────
pub const WNOHANG: u32 = 1;

// ── errno values (from onyx-core/src/errno.rs) ───────────────────────────
// These are the actual negative return codes from OnyxKernel syscalls.
pub const ENOMEM: i64  = -1;
pub const EINVAL: i64  = -2;
pub const ENOENT: i64  = -3;
pub const EIO: i64     = -4;
pub const EPERM: i64   = -5;
pub const ERANGE: i64  = -6;
pub const ENOSYS: i64  = -7;
pub const EBUSY: i64   = -8;
pub const ENOSPC: i64  = -9;
pub const ENOTDIR: i64 = -10;
pub const EISDIR: i64  = -11;
pub const EBADF: i64   = -12;
pub const EEXIST: i64  = -13;
pub const EPIPE: i64   = -14;
pub const EOVERFLOW: i64 = -15;

/// Translate a negative errno return into a human-readable string.
pub fn errno_str(ret: i64) -> &'static [u8] {
    match ret {
        ENOMEM    => b"Out of memory",
        EINVAL    => b"Invalid argument",
        ENOENT    => b"No such file or directory",
        EIO       => b"I/O error",
        EPERM     => b"Permission denied",
        ERANGE    => b"Out of range",
        ENOSYS    => b"Function not implemented",
        EBUSY     => b"Device or resource busy",
        ENOSPC    => b"No space left on device",
        ENOTDIR   => b"Not a directory",
        EISDIR    => b"Is a directory",
        EBADF     => b"Bad file descriptor",
        EEXIST    => b"File exists",
        EPIPE     => b"Broken pipe",
        EOVERFLOW => b"Value too large",
        _         => b"Unknown error",
    }
}

// ── Wrappers ─────────────────────────────────────────────────────────────

/// write(fd, buf, len) → number of bytes written.
/// fd must be 1 (stdout) or 2 (stderr); other fds use write_fd().
#[inline]
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

/// read(fd, buf, len) → number of bytes read (for fd=0, includes line editing).
#[inline]
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

/// exit(code) — terminate the current process. Does not return.
#[inline]
pub unsafe fn exit(code: u64) -> ! {
    asm!(
        "ecall",
        in("a7") SYS_exit,
        in("a0") code,
    );
    loop { asm!("wfi"); }
}

/// yield — give up the CPU to the scheduler.
#[inline]
pub unsafe fn yield_cpu() {
    let _ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_yield,
        lateout("a0") _ret,
    );
}

/// getpid() → current process ID.
#[inline]
pub unsafe fn getpid() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getpid,
        lateout("a0") ret,
    );
    ret
}

/// getring() → 0=kernel, 1=root, 2=user.
#[inline]
pub unsafe fn getring() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getring,
        lateout("a0") ret,
    );
    ret
}

/// getuid() → current user ID.
#[inline]
pub unsafe fn getuid() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_getuid,
        lateout("a0") ret,
    );
    ret
}

/// open(path, flags, mode) → fd token (≥0) or negative errno.
/// `path` must be a NUL-terminated absolute path.
#[inline]
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

/// close(fd) → 0 on success or negative errno.
#[inline]
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

/// pipe(out_fds) → 0 on success. Fills `out_fds[0]` = read end,
/// `out_fds[1]` = write end. Both fds are token-style (u64).
#[inline]
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

/// dup(fd) → new fd token (≥0) or negative errno.
#[inline]
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

/// fork() → child PID to parent, 0 to child, or negative errno.
#[inline]
pub unsafe fn fork() -> i64 {
    let ret: i64;
    asm!(
        "ecall",
        in("a7") SYS_fork,
        lateout("a0") ret,
    );
    ret
}

/// lseek(fd, offset, whence) → new file position or negative errno.
#[inline]
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

/// stat(path, buf) → 0 on success. Fills a 128-byte `struct stat` at `buf`.
#[inline]
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

/// fstat(fd, buf) → 0 on success.
#[inline]
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

/// readdir(dir_path, name_out, len) → 1 if entry read, 0 if no more, <0 on error.
/// Stateful: each call returns the next entry. Resets when a different path is given.
#[inline]
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

/// read(fd, buf, len) for fd ≥ 3 — reads from an open file.
/// (SYS_read with fd ≥ 3 dispatches to vfs::read internally.)
#[inline]
pub unsafe fn read_fd(fd: u64, buf: *mut u8, len: u64) -> i64 {
    read(fd, buf, len)
}

/// write_fd(fd, buf, len) → bytes written (for fd ≥ 3).
#[inline]
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

/// create(path, mode, reserved) → fd token. Root-only.
#[inline]
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

/// mkdir(path) → 0 on success. Root-only.
#[inline]
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

/// unlink(path) → 0 on success. Root-only. Removes a file (not a directory).
#[inline]
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

/// rename(old, new) → 0 on success. Root-only.
#[inline]
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

/// chdir(path) → 0 on success. Updates the process's cwd.
#[inline]
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

/// getcwd(buf, len) → length of cwd string (excl. NUL). Fills `buf` with NUL-terminated cwd.
#[inline]
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

/// exec(path, argv) — replace current process with a new binary.
/// `argv` is a pointer to a NULL-terminated array of `*const u8` string pointers.
/// Does not return on success.
#[inline]
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

/// execve(path, argv, envp) — like exec but with environment.
#[inline]
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

/// spawn(path, argv, ring_hint) → child PID. Root-only.
/// Creates a new child process running `path`. Does not replace the caller.
#[inline]
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

/// wait(status_out) → exited child PID. Blocks until a child exits.
/// If `status_out` is non-null, stores the child's exit code there.
#[inline]
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

/// waitpid(pid, status_out, options) → reaped PID, or 0 with WNOHANG.
/// pid == 0xFFFF_FFFF means "any child".
#[inline]
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

/// uname(buf) → 0 on success. Fills a 390-byte buffer with:
/// sysname[65], nodename[65], release[65], version[65], machine[65].
#[inline]
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

/// clock_gettime(clk_id, ts) → 0 on success. Fills ts[0]=sec, ts[1]=nsec.
#[inline]
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

/// getdents64(fd, buf, count) → bytes written, or 0 at end of directory.
#[inline]
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

/// ioctl(fd, request, arg) → 0 on success or negative errno.
#[inline]
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

/// Enable raw terminal mode for stdin (fd 0). After this, sys_read(0, ...)
/// returns raw bytes without echo or line editing.
#[inline]
pub unsafe fn enable_raw_mode() -> i64 {
    ioctl(0, TIOCSRAW, 0)
}

/// Disable raw terminal mode (restore cooked/line-edited input).
#[inline]
pub unsafe fn disable_raw_mode() -> i64 {
    ioctl(0, TIOCRRAW, 0)
}
