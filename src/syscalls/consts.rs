#![allow(dead_code, non_upper_case_globals)]

pub const SYS_write: u64 = 1;
pub const SYS_read: u64 = 2;
pub const SYS_exit: u64 = 3;
pub const SYS_yield: u64 = 4;
pub const SYS_getpid: u64 = 5;
pub const SYS_sbrk: u64 = 13;
pub const SYS_open: u64 = 8;
pub const SYS_close: u64 = 9;
pub const SYS_lseek: u64 = 10;
pub const SYS_stat: u64 = 11;
pub const SYS_exec: u64 = 12;
pub const SYS_spawn: u64 = 14;
pub const SYS_wait: u64 = 15;
pub const SYS_readdir: u64 = 16;
pub const SYS_getring: u64 = 17;
pub const SYS_dropring: u64 = 18;
pub const SYS_write_fd: u64 = 24;
pub const SYS_create: u64 = 25;
pub const SYS_mkdir: u64 = 26;
pub const SYS_unlink: u64 = 37;
pub const SYS_rename: u64 = 38;
pub const SYS_chdir: u64 = 39;
pub const SYS_getcwd: u64 = 40;
pub const SYS_getuid: u64 = 45;
pub const SYS_uname: u64 = 48;
pub const SYS_fstat: u64 = 50;
pub const SYS_waitpid: u64 = 51;
pub const SYS_getdents64: u64 = 52;
pub const SYS_execve: u64 = 58;
pub const SYS_clock_gettime: u64 = 64;
pub const SYS_isatty: u64 = 66;
pub const SYS_pipe: u64 = 36;
pub const SYS_dup: u64 = 35;
pub const SYS_fork: u64 = 63;
pub const SYS_kill: u64 = 62;
pub const SYS_getdents: u64 = 77;
pub const SYS_ioctl: u64 = 53;

pub const TIOCSRAW: u64 = 0x5421;
pub const TIOCRRAW: u64 = 0x5422;
pub const TIOCGRAW: u64 = 0x5423;

pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;
pub const O_RDWR: u32 = 2;
pub const O_CREAT: u32 = 1 << 6;
pub const O_TRUNC: u32 = 1 << 9;
pub const O_APPEND: u32 = 1 << 10;

pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

pub const WNOHANG: u32 = 1;
pub const SIGCONT: i32 = 18;

pub const ENOMEM: i64 = -1;
pub const EINVAL: i64 = -2;
pub const ENOENT: i64 = -3;
pub const EIO: i64 = -4;
pub const EPERM: i64 = -5;
pub const ERANGE: i64 = -6;
pub const ENOSYS: i64 = -7;
pub const EBUSY: i64 = -8;
pub const ENOSPC: i64 = -9;
pub const ENOTDIR: i64 = -10;
pub const EISDIR: i64 = -11;
pub const EBADF: i64 = -12;
pub const EEXIST: i64 = -13;
pub const EPIPE: i64 = -14;
pub const EOVERFLOW: i64 = -15;

pub fn errno_str(ret: i64) -> &'static [u8] {
    match ret {
        ENOMEM => b"Out of memory",
        EINVAL => b"Invalid argument",
        ENOENT => b"No such file or directory",
        EIO => b"I/O error",
        EPERM => b"Permission denied",
        ERANGE => b"Out of range",
        ENOSYS => b"Function not implemented",
        EBUSY => b"Device or resource busy",
        ENOSPC => b"No space left on device",
        ENOTDIR => b"Not a directory",
        EISDIR => b"Is a directory",
        EBADF => b"Bad file descriptor",
        EEXIST => b"File exists",
        EPIPE => b"Broken pipe",
        EOVERFLOW => b"Value too large",
        _ => b"Unknown error",
    }
}
