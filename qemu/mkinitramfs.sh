#!/usr/bin/env bash
# Build the QEMU device-sim initramfs: the pinned static busybox + qemu/rootfs/.
# The busybox binary is the ONLY external input (sha256-pinned, fail-closed —
# see qemu/lib.sh).
#
# Env:
#   BUSYBOX   path to a local busybox binary (skips the download; still verified)
#   OUT       output dir (default: qemu/out); initramfs at $OUT/initramfs.cpio.gz
set -euo pipefail

QEMU_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=qemu/lib.sh disable=SC1091
. "$QEMU_DIR/lib.sh"
OUT="${OUT:-$QEMU_DIR/out}"

qemu_get_busybox

# Assemble in a scratch dir, removed on exit (scratch-dir leaks were a past
# review finding in this repo).
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/warden-qemu-initramfs.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT
qemu_stage_rootfs "$SCRATCH/root"

# Pack (newc cpio, gzip). No device nodes required: /init mounts devtmpfs and
# reopens the console itself, so the archive builds unprivileged.
( cd "$SCRATCH/root" && find . -print0 | cpio -0 -o -H newc -R +0:+0 2>/dev/null ) \
  | gzip -9 > "$OUT/initramfs.cpio.gz"
qemu_log "initramfs: $OUT/initramfs.cpio.gz ($(du -h "$OUT/initramfs.cpio.gz" | cut -f1))"
