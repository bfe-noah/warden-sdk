#!/usr/bin/env bash
# Boot the warden kernel under qemu-system-arm -M virt and assert the initramfs
# sentinel appears on the console. FAILS CLOSED: a missing zImage, initramfs,
# or qemu binary is an error, never a skip.
#
# Usage: boot-smoke.sh <zImage> [initramfs.cpio.gz]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/

ZIMAGE="${1:-}"
if [ -z "$ZIMAGE" ] || [ ! -f "$ZIMAGE" ]; then
  echo "FATAL: usage: $0 <zImage> [initramfs]: zImage missing or not a file: '${ZIMAGE:-}'" >&2
  exit 1
fi
INITRD="${2:-$QDIR/out/initramfs.cpio.gz}"
[ -f "$INITRD" ] || {
  echo "FATAL: initramfs not found at $INITRD: run qemu/mkinitramfs.sh first" >&2
  exit 1
}
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH (apt-get install qemu-system-arm): see qemu/README.md" >&2
  exit 1
}

LOG="$(mktemp "${TMPDIR:-/tmp}/warden-qemu-smoke.XXXXXX")"
trap 'rm -f "$LOG"' EXIT

# Delegate the qemu invocation to run.sh (--no-disk) so the machine shape
# (-M virt,highmem=off, cpu, memory, virtio topology) lives in exactly one
# place: the two hand-copied invocations had already drifted once.
# timeout -k: a wedged qemu that ignores SIGTERM gets SIGKILLed 10s later
# instead of holding the job until the workflow-level timeout.
timeout -k 10 180 bash "$QDIR/run.sh" \
  --kernel "$ZIMAGE" --initrd "$INITRD" --no-disk \
  --ssh-port 0 --http-port 0 --api-port 0 \
  </dev/null | tee "$LOG" || {
    echo "FATAL: qemu exited non-zero (or hung until the 180s timeout)" >&2
    exit 1
  }

grep -q "WARDEN-QEMU-BOOT-OK" "$LOG" || {
  echo "FATAL: boot sentinel WARDEN-QEMU-BOOT-OK not found in console log" >&2
  exit 1
}
echo "boot smoke OK"
