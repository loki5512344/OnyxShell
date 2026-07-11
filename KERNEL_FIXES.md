# OnyxShell — Critical Kernel Fixes

These are minimal, surgical fixes to OnyxKernel that are technically
necessary for OnyxShell to function correctly. They do not change the
kernel's architecture — they fix three bugs that cause incorrect behavior
during shell operation.

## How to Apply

All fixes are already applied in the `OnyxKernel/` directory included in
this archive. If you are starting from a fresh clone of OnyxKernel, apply
the OnyxInit patches first (from `onyx-init-patches.zip`), then apply
these fixes.

Alternatively, the full diff is in `kernel-fixes.patch` — apply with:

```bash
cd OnyxKernel
git apply ../kernel-fixes.patch
```

## Fix 1: `vfs::create` fd table mismatch

**File:** `kernel/src/fs/vfs/create.rs`

**Problem:** `vfs::create` called `alloc_fd` (which uses `G_KERNEL_FDS`
when `is_kernel_boot()` returns true) but then initialized the fd in
`current().fds` directly via `let fd = &mut p.fds[idx]`. This caused
`EBADF` on subsequent `write_fd` / `read_fd` calls because `fd_check`
and `fd_get` read from a different table than the one `create` wrote to.

**Symptom:** `cp` always fails with "write error" because `write_fd`
returns `EBADF` (-12).

**Fix:** Use `fd_set` / `fd_get` (which respect `is_kernel_boot()`)
instead of accessing `current().fds` directly.

## Fix 2: `sys_uname` user-pointer dereference

**File:** `kernel/src/syscall/fs_sys3/info.rs`

**Problem:** `sys_uname` wrote to the user buffer via
`let out = buf as *mut u8` without translating the VA to a PA. Since the
kernel runs with the user's `satp` but in S-mode, this works only if
`SSTATUS_SUM` is set (which it is during trap entry). However, the
kernel's `translate` function is the correct way to access user memory,
and the direct dereference caused a kernel page fault in some configurations.

**Symptom:** `uname` command causes "KERNEL page fault" and halts.

**Fix:** Call `vmm::translate(proc::current().root_pa, buf)` to get the
PA, then write to the PA.

## Fix 3: `current_pid` state check

**File:** `kernel/src/proc/process/current.rs`

**Problem:** `current_pid` returned 0 when the current process was not
in `Running` state (e.g., if a timer tick had temporarily changed the
state during a syscall). This caused `is_kernel_boot()` to return true,
which made `alloc_fd` use `G_KERNEL_FDS` instead of the process's own
fd table — leading to the same EBADF issue as Fix 1.

**Symptom:** `cp` fails intermittently (depending on timer tick timing).

**Fix:** Return `(*p).pid` regardless of state. The process pointed to
by `G_HART_CURRENT[hart_id()]` is always the "current" process for this
hart, even if its state was momentarily changed by a preempting timer
interrupt.

## Fix 4: OnyxBoot `stdbool.h` (not a kernel fix)

**File:** `OnyxBoot/include/types.h`

**Problem:** `ext4.c` and `fat.c` use `bool` without including
`<stdbool.h>`. GCC 14+ (and newer versions generally) enforce C99 type
correctness and refuse to compile.

**Symptom:** `make -C OnyxBoot` fails with "unknown type name 'bool'".

**Fix:** Add `#include <stdbool.h>` to `include/types.h`.
