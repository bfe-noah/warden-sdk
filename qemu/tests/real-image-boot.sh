#!/usr/bin/env bash
# Real-image milestone: boot an ACTUAL flare-edge build (rootfs.img + oem.img
# matched pair) in the VM on the 6.18 kernel and assert its own init chain
# reaches multi-user: the vendor rcS runs, real warden daemons start, and a
# getty answers on the console.
#
# Documented caveats (this is a fidelity milestone, not full parity): the
# RV1106-only init steps degrade on virt (backlight, goodix, npu, the 5.10
# /oem modules fail vermagic), and binaries older than the flare-edge #106
# fix reproduce that crash faithfully. Interactive login uses the image's own
# credentials — deliberately not recorded here.
#
# FAILS CLOSED on missing prerequisites.
#
# Usage: real-image-boot.sh <zImage> <rootfs.img> <oem.img>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/

ZIMAGE="${1:-}"; ROOTFS="${2:-}"; OEM="${3:-}"
for f in "$ZIMAGE" "$ROOTFS" "$OEM"; do
  if [ -z "$f" ] || [ ! -f "$f" ]; then
    echo "FATAL: usage: $0 <zImage> <rootfs.img> <oem.img> — '$f' missing" >&2
    exit 1
  fi
done
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH — see qemu/README.md" >&2
  exit 1
}

WORK="$(mktemp -d /tmp/wqr.XXXXXX)"
QEMU_PID=""
cleanup() {
  if [ -n "$QEMU_PID" ]; then kill "$QEMU_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

bash "$QDIR/mkinitramfs.sh"
bash "$QDIR/mkimage.sh" --rootfs-image "$ROOTFS" --oem-image "$OEM"

for _attempt in 1 2 3; do
  PORT=$((23000 + RANDOM % 20000))
  : > "$WORK/console.log"
  bash "$QDIR/run.sh" --kernel "$ZIMAGE" \
    --ssh-port "$PORT" --http-port $((PORT + 1)) --api-port $((PORT + 2)) \
    > "$WORK/console.log" 2>&1 &
  QEMU_PID=$!
  sleep 3
  kill -0 "$QEMU_PID" 2>/dev/null && break
  if grep -aq 'Could not set up host forwarding' "$WORK/console.log"; then
    echo "== hostfwd port collision on base $PORT — retrying"
    QEMU_PID=""
    continue
  fi
  echo "FATAL: VM died at launch:" >&2; tail -20 "$WORK/console.log" >&2; exit 1
done
if [ -z "$QEMU_PID" ] || ! kill -0 "$QEMU_PID" 2>/dev/null; then
  echo "FATAL: could not launch the VM after 3 port attempts" >&2
  exit 1
fi

deadline=$((SECONDS + 180))
ok_switch=0 ok_daemons=0 ok_getty=0
while [ $SECONDS -lt $deadline ]; do
  grep -aq 'rc: switching root to rootfs_a' "$WORK/console.log" && ok_switch=1
  [ "$(grep -ac 'Starting warden-' "$WORK/console.log")" -ge 2 ] && ok_daemons=1
  grep -aq 'login:' "$WORK/console.log" && ok_getty=1
  [ $ok_switch -eq 1 ] && [ $ok_daemons -eq 1 ] && [ $ok_getty -eq 1 ] && break
  kill -0 "$QEMU_PID" 2>/dev/null || {
    echo "FATAL: VM exited early" >&2; tail -30 "$WORK/console.log" >&2; exit 1; }
  sleep 3
done

fail=0
[ $ok_switch -eq 1 ]  || { echo "FAIL: never switch_rooted into the real image"; fail=1; }
[ $ok_daemons -eq 1 ] || { echo "FAIL: the image's own init never started warden daemons"; fail=1; }
[ $ok_getty -eq 1 ]   || { echo "FAIL: no getty login prompt on the console"; fail=1; }
[ $fail -eq 0 ] || { tail -25 "$WORK/console.log" >&2; exit 1; }
echo "REAL-IMAGE-BOOT-PASS: the flare-edge image reached multi-user on the VM"
