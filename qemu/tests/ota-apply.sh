#!/usr/bin/env bash
# FULL OTA apply scenario (the loop the desk e2e stops short of): the real
# flared inside the VM downloads a real signed tier-1 .wfw whose payload is a
# bootable rootfs, verifies it, and ACTUALLY WRITES rootfs_b (safe: it is a
# region inside disk.img); the harness then reboots into slot _b and asserts
# the applied firmware version is running.
#
# Documented emulation gaps (ADR-0006): the BCB slot CHOICE and the physical
# reset are performed by the harness (cmdline slot + a fresh qemu boot), not
# by U-Boot/CRU. Those stay bench territory.
#
# FAILS CLOSED on missing prerequisites.
#
# Usage: ota-apply.sh <zImage-virt>
# Env:   FLARE_EDGE  path to a flare-edge checkout (mock portal, mk-wfw, dev key)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/
# mkfs.ext4 lives in sbin (not on user PATH on Debian).
PATH="$PATH:/usr/sbin:/sbin"

ZIMAGE="${1:-}"
if [ -z "$ZIMAGE" ] || [ ! -f "$ZIMAGE" ]; then
  echo "FATAL: usage: $0 <zImage>: the virt.fragment kernel variant" >&2
  exit 1
fi
if [ -z "${FLARE_EDGE:-}" ] || [ ! -f "$FLARE_EDGE/tools/mock-flare-portal.py" ]; then
  echo "FATAL: FLARE_EDGE must point at a flare-edge checkout" >&2
  exit 1
fi
[ -x "$QDIR/payload/warden-flared" ] || {
  echo "FATAL: no qemu/payload/warden-flared (needs the WARDEN_HARD_RESET-gated build)" >&2
  exit 1
}
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH: see qemu/README.md" >&2
  exit 1
}

WORK="$(mktemp -d /tmp/wqo.XXXXXX)"
QEMU_PID="" MOCK_PID=""
cleanup() {
  if [ -n "$QEMU_PID" ]; then kill "$QEMU_PID" 2>/dev/null || true; fi
  if [ -n "$MOCK_PID" ]; then kill "$MOCK_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

DEVICE_ID="$(python3 -c 'import uuid; print(uuid.uuid4())')"
API_KEY="$(python3 -c 'import secrets; print(secrets.token_hex(24))')"
PORT=$((20000 + RANDOM % 20000))

# 0. The offer's payload is a REAL bootable rootfs: the same skeleton the
#    disk uses, stamped with the NEW version: booting it is the proof.
QEMU_DIR="$QDIR"
OUT="$QDIR/out"
# shellcheck source=qemu/lib.sh disable=SC1091
. "$QDIR/lib.sh"
qemu_get_busybox
qemu_stage_rootfs "$WORK/newroot"
printf '0.0.2\n' > "$WORK/newroot/etc/warden-firmware-version"
printf 'applied-via-ota\n' > "$WORK/newroot/etc/ota-marker"
truncate -s 64M "$WORK/rootfs-payload.img"
mkfs.ext4 -F -q -d "$WORK/newroot" "$WORK/rootfs-payload.img"

FW_SIGNING_KEY_FILE="$FLARE_EDGE/tools/testdata/fw-dev-key.seed" \
  WARDEN_KERNEL_VERSION=6.18.46 WARDEN_BUILDROOT_VERSION=2025.02 \
  WARDEN_UBOOT_VERSION=2017.09 \
  bash "$FLARE_EDGE/tools/mk-wfw.sh" "$WORK/rootfs-payload.img" 1 0.0.2 "$WORK/offer.wfw"

# 1. mock portal offering it.
python3 "$FLARE_EDGE/tools/mock-flare-portal.py" \
  --port "$PORT" --device "$DEVICE_ID:$API_KEY" \
  --wfw "$WORK/offer.wfw" > "$WORK/mock.log" 2>&1 &
MOCK_PID=$!
mock_ready=0
for _ in $(seq 1 50); do
  curl -so /dev/null "http://127.0.0.1:$PORT/" && { mock_ready=1; break; }
  kill -0 "$MOCK_PID" 2>/dev/null || { echo "FATAL: mock portal died:" >&2; cat "$WORK/mock.log" >&2; exit 1; }
  sleep 0.2
done
[ "$mock_ready" = 1 ] || { echo "FATAL: mock never answered on :$PORT" >&2; exit 1; }
echo "== mock portal on :$PORT offering 0.0.2 (payload: bootable rootfs, 64M)"

# 2. image at version 0.0.1, enrolment seeded.
bash "$QDIR/mkinitramfs.sh"
bash "$QDIR/mkimage.sh" \
  --portal-url "http://10.0.2.2:$PORT" \
  --state "flare.device_id=$DEVICE_ID" \
  --state "flare.api_key=$API_KEY" \
  --state "flare.site=qemu-devsim" \
  --fw-version 0.0.1

# 3. boot slot _a WITH APPLY ENABLED; flared should pull, verify, write
#    rootfs_b, and surface the (gated) reboot attempt.
QEMU_PID=""
for _attempt in 1 2 3; do
  VMBASE=$((20000 + RANDOM % 20000))
  : > "$WORK/console.log"
  bash "$QDIR/run.sh" --kernel "$ZIMAGE" --allow-apply \
    --ssh-port "$VMBASE" --http-port $((VMBASE + 1)) --api-port $((VMBASE + 2)) \
    > "$WORK/console.log" 2>&1 &
  QEMU_PID=$!
  sleep 3
  kill -0 "$QEMU_PID" 2>/dev/null && break
  if grep -aq 'Could not set up host forwarding' "$WORK/console.log"; then
    echo "== hostfwd port collision on base $VMBASE, retrying"
    QEMU_PID=""
    continue
  fi
  echo "FATAL: VM died at launch:" >&2; tail -20 "$WORK/console.log" >&2; exit 1
done
if [ -z "$QEMU_PID" ] || ! kill -0 "$QEMU_PID" 2>/dev/null; then
  echo "FATAL: could not launch the VM after 3 port attempts" >&2
  exit 1
fi

# 4. wait for the apply to conclude. flared logs to its in-guest file, not
#    the console, but the next check-in REPORTS the outcome to the portal:
#    detail "hard reset failed after slot flip" is the exact post-apply state
#    under the gated reset (write done, AvbABData flipped, reboot refused).
deadline=$((SECONDS + 420))
staged=0
while [ $SECONDS -lt $deadline ]; do
  grep -aq 'hard reset failed after slot flip' "$WORK/mock.log" && { staged=1; break; }
  grep -aq 'WARDEN-QEMU-MOUNT-FAILED' "$WORK/console.log" && {
    echo "FATAL: guest mount failed" >&2; exit 1; }
  kill -0 "$QEMU_PID" 2>/dev/null || {
    echo "FATAL: VM exited early" >&2; tail -30 "$WORK/console.log" >&2; exit 1; }
  sleep 3
done
[ "$staged" = 1 ] || {
  echo "FATAL: apply never reached the post-flip state within 420s; tails:" >&2
  tail -20 "$WORK/console.log" >&2
  tail -10 "$WORK/mock.log" >&2
  exit 1
}
grep -aq "GET /api/v1/devices/$DEVICE_ID/firmware/assets/.* -> 200" "$WORK/mock.log" || {
  echo "FATAL: staged without a portal asset download?!" >&2
  exit 1
}
echo "== apply staged (asset downloaded, rootfs_b written, reset gated), rebooting into _b"
kill "$QEMU_PID" 2>/dev/null || true
wait "$QEMU_PID" 2>/dev/null || true
QEMU_PID=""

# 5. the harness performs the "reboot": boot slot _b, assert the OTA'd rootfs
#    is what runs.
{ sleep 40; printf 'cat /etc/warden-firmware-version /etc/ota-marker\n'; sleep 3; printf 'poweroff -f\n'; sleep 8; } | \
  timeout 180 bash "$QDIR/run.sh" --kernel "$ZIMAGE" --slot _b --shell \
    --ssh-port 0 --http-port 0 --api-port 0 \
    > "$WORK/boot-b.log" 2>&1 || true
grep -aq 'WARDEN-QEMU-ROOTFS-OK slot=_b' "$WORK/boot-b.log" || {
  echo "FATAL: slot _b did not boot; tail:" >&2; tail -25 "$WORK/boot-b.log" >&2; exit 1
}
grep -aq '^0.0.2' "$WORK/boot-b.log" || {
  echo "FATAL: _b is not running the applied 0.0.2 firmware" >&2
  grep -a 'warden-firmware-version' -A2 "$WORK/boot-b.log" >&2 || true
  exit 1
}
grep -aq 'applied-via-ota' "$WORK/boot-b.log" || {
  echo "FATAL: OTA marker missing on _b" >&2; exit 1
}
echo "OTA-APPLY-PASS: 0.0.1 -> 0.0.2 applied over the air and booted from slot _b"
