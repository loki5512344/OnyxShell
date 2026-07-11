#!/bin/bash
# test_qemu.sh — build the full OnyxOS stack with the new OnyxShell and
# launch QEMU for interactive testing.
#
# Usage:
#   ./test_qemu.sh           # interactive (you type commands)
#   ./test_qemu.sh -s        # run a scripted test session and exit

set -e

HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
KERNEL_DIR="$ROOT/OnyxKernel"
BOOT_DIR="$ROOT/OnyxBoot"
OSH_DIR="$HERE"

# ── Build OnyxBoot ────────────────────────────────────────────────────────
echo "==> Building OnyxBoot"
make -C "$BOOT_DIR" CROSS=riscv64-elf clean all 2>&1 | tail -3

# ── Build OnyxKernel + init + tools ───────────────────────────────────────
echo "==> Building OnyxKernel"
cd "$KERNEL_DIR"
cargo build --release -p onyx_kernel --target riscv64gc-unknown-none-elf 2>&1 | tail -3
cargo build --release -p onyx_init --target riscv64gc-unknown-none-elf 2>&1 | tail -3
cargo build --release -p onyx_tools 2>&1 | tail -3

# ── Build OnyxShell ───────────────────────────────────────────────────────
echo "==> Building OnyxShell"
cd "$OSH_DIR"
cargo build --release 2>&1 | tail -3

# ── Convert all userland ELFs → .onx ──────────────────────────────────────
BUILD="$KERNEL_DIR/build"
mkdir -p "$BUILD"
echo "==> Converting userland ELFs → .onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-init" "$BUILD/init.onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-hello" "$BUILD/hello.onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-login" "$BUILD/login.onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$OSH_DIR/target/riscv64gc-unknown-none-elf/release/onyx-osh" "$BUILD/osh.onx"
"$KERNEL_DIR/target/release/elf2onx" --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-passwd" "$BUILD/passwd.onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-useradd" "$BUILD/useradd.onx"
"$KERNEL_DIR/target/release/elf2onx" --ring=1 --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-userdel" "$BUILD/userdel.onx"
"$KERNEL_DIR/target/release/elf2onx" --compress "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-argv-test" "$BUILD/argv_test.onx"

# ── Build OnyxCC (optional) ───────────────────────────────────────────────
ONYXCCDIR="$ROOT/OnyxCompiller"
if [ -f "$ONYXCCDIR/onyxcc.onx" ]; then
    cp "$ONYXCCDIR/onyxcc.onx" "$BUILD/onyxcc.onx"
fi

# ── Generate PSF1 font ────────────────────────────────────────────────────
echo "==> Generating font"
"$KERNEL_DIR/target/release/psfgen" "$BUILD/default.psf"

# ── Create manifest ───────────────────────────────────────────────────────
MANIFEST="$BUILD/manifest.txt"
{
    echo "dir /bin"
    echo "dir /etc"
    echo "dir /etc/init"
    echo "dir /service"
    echo "dir /users"
    echo "dir /font"
    echo "file $BUILD/hello.onx /bin/hello.onx --ring=1"
    echo "file $BUILD/init.onx /bin/init --ring=1"
    echo "file $BUILD/login.onx /bin/login --ring=1"
    echo "file $BUILD/osh.onx /bin/osh"
    echo "file $BUILD/passwd.onx /bin/passwd"
    echo "file $BUILD/useradd.onx /bin/useradd --ring=1"
    echo "file $BUILD/userdel.onx /bin/userdel --ring=1"
    echo "file $BUILD/default.psf /font/default.psf"
    if [ -f "$BUILD/onyxcc.onx" ]; then
        echo "file $BUILD/onyxcc.onx /bin/onyxcc --ring=1"
    fi
    echo "file $BUILD/argv_test.onx /bin/argv_test"
    ONYXCC_TEST_C="$ONYXCCDIR/tests/hello_full.c"
    if [ -f "$ONYXCC_TEST_C" ]; then
        echo "file $ONYXCC_TEST_C /tmp/test.c"
    fi
} > "$MANIFEST"

# ── Create OnyxFS disk image ──────────────────────────────────────────────
echo "==> Creating OnyxFS v2 disk image"
"$KERNEL_DIR/target/release/mkimage" "$MANIFEST" "$BUILD/disk.img"

# ── Create partitioned boot disk ──────────────────────────────────────────
echo "==> Creating partitioned boot disk"
FAT_LBA=2048
dd if=/dev/zero of="$BUILD/boot.img" bs=1M count=64 2>/dev/null
parted -s "$BUILD/boot.img" mklabel msdos 2>/dev/null
parted -s "$BUILD/boot.img" mkpart primary fat32 1MiB 5MiB 2>/dev/null
mkfs.fat -F 32 "$BUILD/boot.img" --offset=$FAT_LBA 2>/dev/null
mcopy -i "$BUILD/boot.img@@$((FAT_LBA * 512))" "$KERNEL_DIR/target/riscv64gc-unknown-none-elf/release/onyx-kernel" ::kernel.elf 2>/dev/null
SLBA=10240
dd if="$BUILD/disk.img" of="$BUILD/boot.img" bs=512 seek=$SLBA conv=notrunc 2>/dev/null

# ── Launch QEMU ───────────────────────────────────────────────────────────
QEMU_BIN="${QEMU:-qemu-system-riscv64}"
echo "==> Starting QEMU"

if [ "$1" = "-s" ]; then
    # Scripted test: pipe commands then kill QEMU after the last command.
    (
        sleep 6   # wait for kernel boot + login
        printf 'help\n'              ; sleep 1
        printf 'pwd\n'               ; sleep 1
        printf 'ls /\n'              ; sleep 1
        printf 'ls /bin\n'           ; sleep 1
        printf 'ls -l /bin\n'        ; sleep 1
        printf 'mkdir /tmp\n'        ; sleep 1
        printf 'touch /tmp/test.txt\n' ; sleep 1
        printf 'echo hello from osh\n' ; sleep 1
        printf 'cat /etc/passwd\n'   ; sleep 1
        printf 'stat /bin/osh\n'     ; sleep 1
        printf 'whoami\n'            ; sleep 1
        printf 'uname\n'             ; sleep 1
        printf 'ver\n'               ; sleep 1
        printf 'cd /tmp\n'           ; sleep 1
        printf 'pwd\n'               ; sleep 1
        printf 'cd /\n'              ; sleep 1
        printf 'cp /etc/passwd /tmp/copy.txt\n' ; sleep 1
        printf 'cat /tmp/copy.txt\n' ; sleep 1
        printf 'mv /tmp/copy.txt /tmp/moved.txt\n' ; sleep 1
        printf 'ls /tmp\n'           ; sleep 1
        printf 'rm /tmp/test.txt\n'  ; sleep 1
        printf 'rm /tmp/moved.txt\n' ; sleep 1
        printf 'ls /tmp\n'           ; sleep 1
        printf 'exit\n'              ; sleep 2
    ) | timeout 60 "$QEMU_BIN" \
        -M virt -m 256M -smp 1 \
        -bios "$BOOT_DIR/bootloader.bin" \
        -drive file="$BUILD/boot.img",format=raw,if=none,id=drive0 \
        -device virtio-blk-device,drive=drive0 \
        -nographic -no-reboot
else
    exec "$QEMU_BIN" \
        -M virt -m 256M -smp 1 \
        -bios "$BOOT_DIR/bootloader.bin" \
        -drive file="$BUILD/boot.img",format=raw,if=none,id=drive0 \
        -device virtio-blk-device,drive=drive0 \
        -nographic -no-reboot
fi
