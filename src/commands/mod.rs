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

mod exec;
mod file;
mod nav;
mod sys;

pub use exec::script::do_script;

/// Maximum number of arguments per command line.
pub const MAX_ARGS: usize = 16;

/// Maximum input line length (matches main.rs).
pub(crate) const LINE_MAX: usize = 256;

/// Dispatch a tokenized command line.
pub fn dispatch(args: &[&[u8]]) {
    unsafe {
        features::job_reap();
    }
    if args.is_empty() {
        return;
    }
    let cmd = args[0];
    let rest = &args[1..];

    if cmd == b"help" || cmd == b"?" {
        sys::cmd_help(rest);
    } else if cmd == b"echo" {
        sys::cmd_echo(rest);
    } else if cmd == b"export" {
        sys::cmd_export(rest);
    } else if cmd == b"set" {
        sys::cmd_set(rest);
    } else if cmd == b"unset" {
        sys::cmd_unset(rest);
    } else if cmd == b"pwd" {
        nav::cmd_pwd(rest);
    } else if cmd == b"cd" {
        nav::cmd_cd(rest);
    } else if cmd == b"ls" {
        nav::cmd_ls(rest);
    } else if cmd == b"cat" {
        file::cmd_cat(rest);
    } else if cmd == b"rm" {
        file::cmd_rm(rest);
    } else if cmd == b"mkdir" {
        file::cmd_mkdir(rest);
    } else if cmd == b"cp" {
        file::cmd_cp(rest);
    } else if cmd == b"mv" {
        file::cmd_mv(rest);
    } else if cmd == b"touch" {
        file::cmd_touch(rest);
    } else if cmd == b"stat" {
        file::cmd_stat(rest);
    } else if cmd == b"whoami" {
        sys::cmd_whoami(rest);
    } else if cmd == b"uname" {
        sys::cmd_uname(rest);
    } else if cmd == b"clear" {
        sys::cmd_clear(rest);
    } else if cmd == b"exit" || cmd == b"logout" {
        sys::cmd_exit(rest);
    } else if cmd == b"jobs" {
        exec::cmd_jobs(rest);
    } else if cmd == b"fg" {
        exec::cmd_fg(rest);
    } else if cmd == b"bg" {
        exec::cmd_bg(rest);
    } else if cmd == b"exec" {
        exec::cmd_exec(rest);
    } else if cmd == b"run" {
        exec::cmd_run(rest);
    } else if cmd == b"source" || cmd == b"." {
        exec::cmd_source(rest);
    } else if cmd == b"date" {
        sys::cmd_date(rest);
    } else if cmd == b"ver" || cmd == b"version" {
        sys::cmd_ver(rest);
    } else {
        io::write_raw(b"osh: ");
        io::write_raw(cmd);
        io::write_raw(b": command not found (try 'help')\n");
    }
}
