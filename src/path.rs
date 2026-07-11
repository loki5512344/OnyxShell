//! Path resolution — convert relative paths to absolute paths.
//!
//! OnyxKernel's VFS requires all paths passed to `open`, `stat`,
//! `unlink`, `mkdir`, `rename`, etc. to be **absolute** (starting
//! with `/`). The shell accepts relative paths from the user and
//! resolves them against the current working directory before
//! calling syscalls.

use crate::io;
use crate::syscalls;

/// Maximum path length supported by the shell.
pub const PATH_MAX: usize = 256;

/// Resolve `input` into an absolute, NUL-terminated path in `out`.
///
/// Rules:
/// - If `input` starts with `/`, it is already absolute — copied as-is.
/// - If `input` starts with `./`, the `./` is stripped and the rest
///   is appended to the cwd.
/// - Otherwise, the input is appended to the cwd with a `/` separator.
/// - `..` and `.` components are handled lexically (without hitting the
///   filesystem): `..` pops the last component, `.` is a no-op.
/// - Special case: cwd `/` does not get a double slash.
///
/// Returns the length of the resolved path (excluding the NUL terminator),
/// or `0` on error (buffer too small or invalid input). On success, the
/// buffer is NUL-terminated.
pub fn resolve(input: &[u8], out: &mut [u8; PATH_MAX]) -> usize {
    if input.is_empty() {
        return 0;
    }

    // If input is absolute, resolve it directly (still normalize . and ..).
    // We do this BEFORE calling getcwd to avoid any getcwd issues with
    // stale buffer state.
    if input[0] == b'/' {
        return normalize(input, out);
    }

    // Get the current working directory from the kernel.
    let mut cwd = [0u8; PATH_MAX];
    let cwd_ret = unsafe { syscalls::getcwd(cwd.as_mut_ptr(), PATH_MAX as u64) };

    if cwd_ret <= 0 {
        // getcwd failed — default to "/".
        return copy_abs(b"/", out);
    }
    let cwd_len = cwd_ret as usize;
    if cwd[0] != b'/' {
        return copy_abs(b"/", out);
    }
    let cwd = &cwd[..cwd_len];

    // Strip a leading "./" from the input.
    let input = if input.len() >= 2 && input[0] == b'.' && input[1] == b'/' {
        &input[2..]
    } else {
        input
    };

    // Join cwd + "/" + input, then normalize.
    let mut joined = [0u8; PATH_MAX];
    // Copy cwd.
    if cwd_len > PATH_MAX - 2 {
        return 0;
    }
    joined[..cwd_len].copy_from_slice(cwd);
    let mut jlen = cwd_len;

    // Add separator if cwd doesn't end with '/'.
    if jlen > 0 && joined[jlen - 1] != b'/' {
        if jlen >= PATH_MAX - 1 { return 0; }
        joined[jlen] = b'/';
        jlen += 1;
    }

    // Append input.
    if jlen + input.len() >= PATH_MAX {
        return 0;
    }
    joined[jlen..jlen + input.len()].copy_from_slice(input);
    jlen += input.len();

    normalize(&joined[..jlen], out)
}

/// Copy an absolute path into `out`, NUL-terminated.
fn copy_abs(path: &[u8], out: &mut [u8; PATH_MAX]) -> usize {
    if path.len() >= PATH_MAX {
        return 0;
    }
    out[..path.len()].copy_from_slice(path);
    out[path.len()] = 0;
    path.len()
}

/// Normalize an absolute path: resolve `.` and `..` components lexically.
///
/// Example: `/a/b/../c/./d` → `/a/c/d`
fn normalize(path: &[u8], out: &mut [u8; PATH_MAX]) -> usize {
    if path.is_empty() || path[0] != b'/' {
        return 0;
    }

    // We build the normalized path in `out` directly, component by component.
    let mut olen = 1usize; // current length of `out` (excluding NUL)
    out[0] = b'/';

    let mut i = 1usize; // skip leading '/'
    let plen = path.len();

    while i < plen {
        // Find the next component (up to the next '/').
        let start = i;
        while i < plen && path[i] != b'/' {
            i += 1;
        }
        let comp = &path[start..i];

        // Skip empty components (consecutive slashes) and ".".
        if comp.is_empty() || comp == b"." {
            // Skip.
        } else if comp == b".." {
            // Pop the last component from `out`.
            // Don't go below "/".
            if olen > 1 {
                olen -= 1; // remove trailing '/' or last char
                while olen > 1 && out[olen - 1] != b'/' {
                    olen -= 1;
                }
            }
            // If olen == 1 (root), stay at root.
        } else {
            // Append the component.
            // Ensure there's a '/' separator (unless we're at root and
            // out already ends with '/').
            if olen > 0 && out[olen - 1] != b'/' {
                if olen >= PATH_MAX - 1 { return 0; }
                out[olen] = b'/';
                olen += 1;
            }
            if olen + comp.len() >= PATH_MAX { return 0; }
            out[olen..olen + comp.len()].copy_from_slice(comp);
            olen += comp.len();
        }

        // Skip the '/'.
        if i < plen && path[i] == b'/' {
            i += 1;
        }
    }

    // If the path ended up empty (shouldn't happen), default to "/".
    if olen == 0 {
        out[0] = b'/';
        olen = 1;
    }

    out[olen] = 0;
    olen
}

/// Print the resolved path for debugging (used by `cd` with no args to show cwd).
#[allow(dead_code)]
pub fn print_cwd() {
    let mut buf = [0u8; PATH_MAX];
    let n = unsafe { syscalls::getcwd(buf.as_mut_ptr(), PATH_MAX as u64) };
    if n > 0 {
        io::write_raw(&buf[..n as usize]);
    } else {
        io::write_str("/");
    }
    io::newline();
}
