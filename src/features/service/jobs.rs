//! Background job management (flat array, sequential IDs).

use crate::io;
use crate::syscalls;

pub const JOB_MAX: usize = 16;

static mut G_JOB_IDS: [usize; JOB_MAX] = [0; JOB_MAX];
static mut G_JOB_PIDS: [i32; JOB_MAX] = [0; JOB_MAX];
static mut G_JOB_RUNNING: [bool; JOB_MAX] = [false; JOB_MAX];
static mut G_JOB_NEXT_ID: usize = 1;
static mut G_JOB_COUNT: usize = 0;

pub unsafe fn job_add(pid: i32) -> usize {
    if G_JOB_COUNT >= JOB_MAX {
        return 0;
    }
    let idx = G_JOB_COUNT;
    let job_id = G_JOB_NEXT_ID;
    G_JOB_NEXT_ID = G_JOB_NEXT_ID.wrapping_add(1);
    G_JOB_IDS[idx] = job_id;
    G_JOB_PIDS[idx] = pid;
    G_JOB_RUNNING[idx] = true;
    G_JOB_COUNT += 1;
    job_id
}

pub unsafe fn job_remove_by_id(job_id: usize) -> bool {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            for j in i..G_JOB_COUNT - 1 {
                G_JOB_IDS[j] = G_JOB_IDS[j + 1];
                G_JOB_PIDS[j] = G_JOB_PIDS[j + 1];
                G_JOB_RUNNING[j] = G_JOB_RUNNING[j + 1];
            }
            G_JOB_COUNT -= 1;
            return true;
        }
    }
    false
}

pub unsafe fn job_find_by_id(job_id: usize) -> Option<(usize, i32, bool)> {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            return Some((G_JOB_IDS[i], G_JOB_PIDS[i], G_JOB_RUNNING[i]));
        }
    }
    None
}

pub unsafe fn job_set_running(job_id: usize, running: bool) -> bool {
    for i in 0..G_JOB_COUNT {
        if G_JOB_IDS[i] == job_id {
            G_JOB_RUNNING[i] = running;
            return true;
        }
    }
    false
}

#[allow(dead_code)]
pub unsafe fn job_count() -> usize {
    G_JOB_COUNT
}

#[allow(dead_code)]
pub unsafe fn job_get_by_index(idx: usize) -> Option<(usize, i32, bool)> {
    if idx >= G_JOB_COUNT {
        return None;
    }
    Some((G_JOB_IDS[idx], G_JOB_PIDS[idx], G_JOB_RUNNING[idx]))
}

pub unsafe fn job_list() {
    for i in 0..G_JOB_COUNT {
        io::write_raw(b"[");
        io::write_u64(G_JOB_IDS[i] as u64);
        io::write_raw(b"] ");
        io::write_i64(G_JOB_PIDS[i] as i64);
        io::write_raw(b" ");
        if G_JOB_RUNNING[i] {
            io::write_raw(b"Running");
        } else {
            io::write_raw(b"Done");
        }
        io::newline();
    }
}

pub unsafe fn job_reap() {
    loop {
        let mut status: i32 = 0;
        let pid = syscalls::waitpid(0xFFFF_FFFF, &mut status, syscalls::WNOHANG);
        if pid <= 0 {
            break;
        }
        let mut found = false;
        for i in 0..G_JOB_COUNT {
            if G_JOB_PIDS[i] as i64 == pid {
                io::write_raw(b"[");
                io::write_u64(G_JOB_IDS[i] as u64);
                io::write_raw(b"] ");
                io::write_i64(pid);
                io::write_raw(b" Done\n");
                for j in i..G_JOB_COUNT - 1 {
                    G_JOB_IDS[j] = G_JOB_IDS[j + 1];
                    G_JOB_PIDS[j] = G_JOB_PIDS[j + 1];
                    G_JOB_RUNNING[j] = G_JOB_RUNNING[j + 1];
                }
                G_JOB_COUNT -= 1;
                found = true;
                break;
            }
        }
    }
}
