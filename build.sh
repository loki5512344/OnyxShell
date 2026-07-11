#!/bin/bash
# build.sh — compile OnyxShell and convert to OnyxExec v2 (.onx) format.
#
# Usage:
#   ./build.sh
#
# Prerequisites:
#   - Rust nightly with riscv64gc-unknown-none-elf target
#   - elf2onx tool (built from OnyxKernel/tools)
#
# Output:
#   build/onyx-osh   — raw RISC-V ELF
#   build/osh.onx    — OnyxExec v2 binary (place at /bin/osh in the disk image)

set -e

HERE="$(cd "$(dirname "$0")" && pwd)"
ONYXKERNEL_DIR="${ONYXKERNEL_DIR:-$(cd "$HERE/../OnyxKernel" && pwd)}"
ELF2ONX="$ONYXKERNEL_DIR/target/release/elf2onx"

# Build the shell (release mode, riscv64gc target).
echo "==> Building OnyxShell"
cd "$HERE"
cargo build --release 2>&1 | tail -5

ELF="$HERE/target/riscv64gc-unknown-none-elf/release/onyx-osh"

if [ ! -f "$ELF" ]; then
    echo "ERROR: $ELF not found — build failed?"
    exit 1
fi

# Prepare output directory.
mkdir -p "$HERE/build"
cp "$ELF" "$HERE/build/onyx-osh"

# Check for elf2onx.
if [ ! -f "$ELF2ONX" ]; then
    echo "==> elf2onx not found at $ELF2ONX"
    echo "    Building elf2onx from OnyxKernel..."
    (cd "$ONYXKERNEL_DIR" && cargo build --release -p onyx_tools 2>&1 | tail -3)
fi

if [ ! -f "$ELF2ONX" ]; then
    echo "ERROR: elf2onx still not found. Build OnyxKernel tools first:"
    echo "  cd $ONYXKERNEL_DIR && cargo build --release -p onyx_tools"
    exit 1
fi

# Convert ELF → OnyxExec v2 (compressed, ring 1 = root space).
# --ring=1 is required so the shell retains root privileges after
# /bin/login execs it. Without --ring=1, exec drops the process
# to ring 2 (user space), and file-mutation commands (rm, mkdir,
# cp, mv, touch) will fail with EPERM.
echo "==> Converting ELF → osh.onx"
"$ELF2ONX" --ring=1 --compress "$ELF" "$HERE/build/osh.onx"

echo "==> Done: $HERE/build/osh.onx"
ls -la "$HERE/build/osh.onx"
