<p align="center">
  <img src="https://img.shields.io/badge/platform-RISC--V%2064--bit-green" alt="RISC-V 64">
  <img src="https://img.shields.io/badge/language-Rust%2090%25-orange" alt="Rust 90%">
  <img src="https://img.shields.io/badge/version-v0.2-blue" alt="v0.2">
  <img src="https://img.shields.io/badge/target-OnyxOS-yellow" alt="OnyxOS">
  <img src="https://img.shields.io/badge/license-GPL--3.0-red" alt="GPL-3.0">
</p>

<p align="center">
<pre class="not-prose" style="text-align:center;font-family:monospace;">
    ███████                                     
  ███▒▒▒▒▒███                                   
 ███     ▒▒███ ████████   █████ ████ █████ █████
▒███      ▒███▒▒███▒▒███ ▒▒███ ▒███ ▒▒███ ▒▒███ 
▒███      ▒███ ▒███ ▒███  ▒███ ▒███  ▒▒▒█████▒  
▒▒███     ███  ▒███ ▒███  ▒███ ▒███   ███▒▒▒███ 
 ▒▒▒███████▒   ████ █████ ▒▒███████  █████ █████
   ▒▒▒▒▒▒▒    ▒▒▒▒ ▒▒▒▒▒   ▒▒▒▒▒███ ▒▒▒▒▒ ▒▒▒▒▒ 
                           ███ ▒███             
                          ▒▒██████              
                           ▒▒▒▒▒▒               
  █████████  █████               ████  ████     
 ███▒▒▒▒▒███▒▒███               ▒▒███ ▒▒███     
▒███    ▒▒▒  ▒███████    ██████  ▒███  ▒███     
▒▒█████████  ▒███▒▒███  ███▒▒███ ▒███  ▒███     
 ▒▒▒▒▒▒▒▒███ ▒███ ▒███ ▒███████  ▒███  ▒███     
 ███    ▒███ ▒███ ▒███ ▒███▒▒▒   ▒███  ▒███     
▒▒█████████  ████ █████▒▒██████  █████ █████    
 ▒▒▒▒▒▒▒▒▒  ▒▒▒▒ ▒▒▒▒▒  ▒▒▒▒▒▒  ▒▒▒▒▒ ▒▒▒▒▒     
                                                
                                                
                                                
</pre>
</p>

<p align="center"><em>A user-space shell for OnyxOS with built-in file operations, navigation, and system commands</em></p>

----

OnyxShell (`/bin/osh`) is the default command-line shell for
[OnyxOS](https://github.com/loki5512344/OnyxKernel). It is a freestanding
RISC-V 64-bit binary written in 90% Rust (`no_std`, `no_main`) that compiles
to the OnyxExec v2 format and runs in ring 1 (root space) when launched by
`/bin/login`.

The shell provides built-in implementations of the most common Unix commands —
`ls`, `cat`, `rm`, `cd`, `cp`, `mv`, `mkdir` — plus `touch`, `stat`, `pwd`,
`echo`, `whoami`, `uname`, `date`, `clear`, `help`, `exit`, `exec`, `run`, and
`ver`. No external binaries are required for basic file management.

Part of the [OnyxOS](https://github.com/loki5512344/OnyxKernel) ecosystem.

----

## Key Features

- **90% Rust** — `no_std`, `no_main`, compiled with `riscv64gc-unknown-none-elf`
- **OnyxExec v2 format** — compressed with `elf2onx --ring=1 --compress`
- **20 built-in commands** — `ls`, `cat`, `cp`, `mv`, `rm`, `mkdir`, `touch`,
  `stat`, `cd`, `pwd`, `echo`, `whoami`, `uname`, `date`, `clear`, `help`,
  `exit`, `exec`, `run`, `ver`
- **Path resolution** — relative paths (`foo`, `./bar`, `../baz`) resolved
  against the kernel's cwd via `getcwd`; `.` and `..` normalized lexically
- **Long-format `ls -l`** — shows file type, permissions, size, and name
- **Detailed `stat`** — displays inode, mode, size, blocks, timestamps, and
  more from the kernel's Linux-compatible `struct stat`
- **External program execution** — `exec` replaces the shell; `run` spawns a
  child process and waits (root-only, via `SYS_spawn` + `SYS_wait`)
- **Error reporting** — all file-mutation commands translate kernel errno
  codes to human-readable messages
- **Line editing** — the kernel's UART driver provides backspace and echo;
  the shell just reads complete lines via `SYS_read`

----

## Built-in Commands

| Command | Description | Root-only? |
|---------|-------------|:----------:|
| `ls [path] [-l]` | List directory contents (use `-l` for long format) | |
| `cat <file>...` | Print file contents to stdout | |
| `cp <src> <dst>` | Copy a file | ✓ |
| `mv <src> <dst>` | Move or rename a file (uses `rename`, falls back to copy+remove) | ✓ |
| `rm <file>...` | Remove (unlink) a file | ✓ |
| `mkdir <dir>...` | Create a directory | ✓ |
| `touch <file>...` | Create an empty file (no error if it exists) | ✓ |
| `stat <file>` | Show file metadata (inode, size, mode, timestamps) | |
| `cd [path]` | Change working directory (default: `/`) | |
| `pwd` | Print working directory | |
| `echo [text]` | Print text followed by a newline | |
| `whoami` | Print current user (uid) and privilege ring | |
| `uname` | Print system information (sysname, nodename, release, version, machine) | |
| `date` | Print current epoch time (seconds + nanoseconds) | |
| `clear` | Clear the terminal screen (ANSI escape) | |
| `help` | List all available commands | |
| `exec <path> [args]` | Replace the shell process with a binary | |
| `run <path> [args]` | Spawn a binary as a child and wait for it | ✓ |
| `exit` | Exit the shell (calls `SYS_exit(0)`) | |
| `ver` | Print shell version and copyright | |

**Root-only** commands require ring 1 (root space). The default first-boot
auto-login is root, so all commands work out of the box. Regular users (ring 2)
will get `Permission denied` from file-mutation commands.

----

## Project Structure

```
osh/
├── Cargo.toml           — package definition (no_std, no_main)
├── .cargo/config.toml   — RISC-V target + linker flags
├── linker.ld            — linker script (page-aligned .bss, entry at 0x10000)
├── build.rs             — passes linker script to rustc
├── src/
│   ├── main.rs          — _start entry point + main REPL loop
│   ├── syscalls.rs      — ecall wrappers for the OnyxKernel syscall ABI
│   ├── io.rs            — write_str / write_u64 / write_hex / read_line
│   ├── path.rs          — relative-to-absolute path resolution + normalization
│   └── commands.rs      — all 20 command implementations
├── build.sh             — builds the shell + converts ELF → osh.onx
├── test_qemu.sh         — full-stack QEMU test (builds kernel, boot, disk)
└── README.md            — this file
```

----

## Building

### Prerequisites

| Tool | Version | Purpose |
|------|---------|---------|
| Rust nightly | ≥ 1.85 | Shell compilation |
| `riscv64gc-unknown-none-elf` target | — | Cross-compilation |
| `elf2onx` | — | ELF → OnyxExec v2 conversion (from OnyxKernel/tools) |
| `qemu-system-riscv64` | — | Testing (optional) |

### Install Rust target

```console
$ rustup target add riscv64gc-unknown-none-elf
```

### Build the shell

```console
$ ./build.sh
```

This produces `build/osh.onx` — a compressed OnyxExec v2 binary tagged as
ring 1 (root space). Place this file at `/bin/osh` in your OnyxFS disk image.

### Manual build

```console
$ cargo build --release
$ elf2onx --ring=1 --compress target/riscv64gc-unknown-none-elf/release/onyx-osh build/osh.onx
```

> **Why `--ring=1`?**
>
> OnyxKernel's `exec` syscall sets the new process's ring based on the binary's
> RING1 flag. Without `--ring=1`, `/bin/login` (ring 1) execs `/bin/osh` and
> the shell is dropped to ring 2 (user space). In ring 2, file-mutation
> syscalls (`unlink`, `mkdir`, `create`, `rename`) return `EPERM`, so
> `rm`, `mkdir`, `cp`, `mv`, and `touch` would fail.

----

## Testing in QEMU

The `test_qemu.sh` script builds the entire OnyxOS stack (OnyxBoot +
OnyxKernel + OnyxShell) and launches QEMU:

```console
$ ./test_qemu.sh          # interactive mode — type commands at osh$ prompt
$ ./test_qemu.sh -s       # scripted mode — runs a test suite and exits
```

### Prerequisites for QEMU testing

- OnyxBoot, OnyxKernel, and OnyxCompiller repos cloned as siblings of `osh/`
- `riscv64-elf-gcc` (or `riscv64-unknown-elf-gcc`) for OnyxBoot
- `qemu-system-riscv64`, `parted`, `mkfs.fat`, `mcopy`

### Expected boot output

```
OnyxBoot v0.4 [riscv-virtio,qemu]
...
[kernel] OnyxKernel v0.4 — RISC-V 64-bit
...
[init] OnyxOS init v0.4 (service manager)
[init] launching /bin/login
[login] no users found - auto-login as root
[login] launching /bin/osh (root, ring 1)
OnyxShell v0.2.0 (built-in commands)
osh$ _
```

### Example session

```
osh$ help
osh$ ls /
osh$ ls -l /bin
osh$ cat /etc/passwd
osh$ mkdir /tmp
osh$ touch /tmp/test.txt
osh$ cp /etc/passwd /tmp/copy.txt
osh$ cat /tmp/copy.txt
osh$ mv /tmp/copy.txt /tmp/moved.txt
osh$ ls /tmp
osh$ rm /tmp/test.txt
osh$ rm /tmp/moved.txt
osh$ stat /bin/osh
osh$ whoami
osh$ uname
osh$ cd /tmp
osh$ pwd
osh$ exit
```

----

## How It Works

### Boot Flow

1. **OnyxBoot** (M-mode) loads `kernel.elf` from the VirtIO disk and jumps to
   `kmain`.
2. **OnyxKernel** initializes hardware, mounts OnyxFS, and spawns `/bin/init`
   (PID 1, ring 1).
3. **`/bin/init`** (OnyxInit service manager) scans `/service/`, then spawns
   `/bin/login`.
4. **`/bin/login`** auto-logs in as root on first boot (no users in
   `/etc/passwd`), then execs `/bin/osh`.
5. **`/bin/osh`** (this shell) enters its read-eval-print loop.

### Syscall ABI

The shell communicates with the kernel via RISC-V `ecall` instructions. Each
syscall wrapper in `src/syscalls.rs` is a thin inline-assembly block that
loads the syscall number into `a7`, arguments into `a0`–`a2`, executes
`ecall`, and reads the return value from `a0`.

The shell uses these syscalls:

| Syscall | Number | Purpose |
|---------|--------|---------|
| `write` | 1 | Write to stdout (fd 1) |
| `read` | 2 | Read from stdin (fd 0) — kernel handles line editing |
| `exit` | 3 | Terminate the shell |
| `open` | 8 | Open a file (O_RDONLY, O_CREAT, O_WRONLY, O_TRUNC) |
| `close` | 9 | Close a file descriptor |
| `stat` | 11 | Get file metadata (128-byte `struct stat`) |
| `readdir` | 16 | Read next directory entry (stateful, path-based) |
| `getring` | 17 | Get current privilege ring (0/1/2) |
| `write_fd` | 24 | Write to a file descriptor (fd ≥ 3) |
| `create` | 25 | Create a new regular file (root-only) |
| `mkdir` | 26 | Create a directory (root-only) |
| `unlink` | 37 | Remove a file (root-only) |
| `rename` | 38 | Rename/move a file (root-only) |
| `chdir` | 39 | Change working directory |
| `getcwd` | 40 | Get current working directory |
| `getuid` | 45 | Get current user ID |
| `uname` | 48 | Get system information |
| `spawn` | 14 | Spawn a child process (root-only) |
| `wait` | 15 | Wait for a child to exit (root-only) |
| `exec` | 12 | Replace process with a new binary |
| `clock_gettime` | 64 | Get current time |

### Path Resolution

OnyxKernel's VFS requires all paths passed to `open`, `stat`, `unlink`,
`mkdir`, `rename`, etc. to be **absolute** (starting with `/`). The shell
accepts relative paths from the user and resolves them in `src/path.rs`:

1. If the path starts with `/`, it is already absolute — normalize `.` and
   `..` components lexically.
2. Otherwise, fetch the cwd via `getcwd`, join `cwd + "/" + path`, then
   normalize.
3. `.` components are skipped; `..` pops the last component.

### Memory Layout

The shell is a freestanding binary with no heap allocation. All buffers are
stack-allocated with fixed sizes:

- Input line: 256 bytes
- Path buffer: 256 bytes
- Stat buffer: 256 bytes
- Read/write buffer: 512 bytes
- Token array: 16 tokens × (offset, length)

The linker script places `.text` at `0x10000` (USER_BASE), `.rodata` after
it, and page-aligns `.bss` so it does not share a page with `.rodata` (which
would cause a page fault on the first `.bss` write).

----

## Integration with OnyxKernel

To integrate the shell into your OnyxKernel build:

1. **Build the shell:**
   ```console
   $ cd osh && ./build.sh
   ```

2. **Copy `build/osh.onx` to your OnyxKernel build directory.**

3. **Update `scripts/run_qemu.sh`** to use the new `osh.onx`:
   ```bash
   elf2onx --ring=1 --compress $OSH_DIR/target/.../onyx-osh $BUILD/osh.onx
   ```

4. **Run QEMU:**
   ```console
   $ ./scripts/run_qemu.sh
   ```

Alternatively, use the included `test_qemu.sh` which automates the entire
build-and-test cycle.

### Kernel patches applied

This shell was tested with the following patches applied to OnyxKernel
(from `onyx-init-patches.zip`):

1. `01-init-service-manager.patch` — rewrites init as a service manager
2. `02-login-auto-root.patch` — auto-login as root on first boot
3. `03-auth-plaintext.patch` — plaintext password storage
4. `04-linker-bss-page-align.patch` — page-align `.bss` in init's linker.ld
5. `05-run-qemu-manifest.patch` — conditional manifest entries in run_qemu.sh

### Critical kernel fixes

Two bugs in OnyxKernel were fixed to make the shell work correctly. These are
minimal, surgical fixes that do not change the kernel's architecture:

1. **`vfs::create` fd table mismatch** (`kernel/src/fs/vfs/create.rs`) —
   `create` called `alloc_fd` (which uses `G_KERNEL_FDS` when
   `is_kernel_boot()` is true) but then initialized the fd in
   `current().fds` directly. This caused `EBADF` on subsequent `write_fd`
   calls because `fd_check` read from a different table than the one
   `create` wrote to. Fixed by using `fd_set` / `fd_get` (which respect
   `is_kernel_boot()`) instead of accessing `current().fds` directly.

2. **`sys_uname` user-pointer dereference** (`kernel/src/syscall/fs_sys3/info.rs`) —
   `sys_uname` wrote to the user buffer via `buf as *mut u8` without
   translating the VA to a PA, causing a kernel page fault. Fixed by
   calling `vmm::translate` first.

3. **`current_pid` state check** (`kernel/src/proc/process/current.rs`) —
   `current_pid` returned 0 when the current process was not in `Running`
   state, causing `is_kernel_boot()` to return true during syscalls
   (if a timer tick had changed the state). This made `alloc_fd` use
   `G_KERNEL_FDS` instead of the process's own fd table. Fixed by
   returning `(*p).pid` regardless of state.

4. **OnyxBoot `stdbool.h`** (`OnyxBoot/include/types.h`) — added
   `#include <stdbool.h>` so `bool` is defined for `ext4.c` and `fat.c`.
   Required for GCC 14+ which enforces C99 type correctness.

----

## Roadmap

- [ ] Tab completion for file paths
- [ ] Command history (up/down arrows)
- [ ] Pipe (`|`) and redirect (`>`, `<`) operators
- [ ] Wildcard globbing (`*`, `?`)
- [ ] Environment variables (`$HOME`, `$PATH`)
- [ ] Background processes (`&`)
- [ ] Shell scripts (batch file execution)

----

## Related Projects

| Project | Description |
|---------|-------------|
| [OnyxKernel](https://github.com/loki5512344/OnyxKernel) | RISC-V 64-bit operating system kernel |
| [OnyxBoot](https://github.com/loki5512344/OnyxBoot) | Minimalist RISC-V 64-bit bootloader |
| [OnyxCompiller](https://github.com/loki5512344/OnyxCompiller) | C → RV64 compiler (runs on OnyxOS) |

----

## License

GPL-3.0-or-later — same as OnyxKernel.
