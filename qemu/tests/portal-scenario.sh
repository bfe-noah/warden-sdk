#!/usr/bin/env bash
# End-to-end device scenario: the REAL warden-flared, running inside the VM,
# checks in to flare-edge's mock FLARE portal on the host and pulls its
# firmware desired-state: the exact device-initiated HTTPS(-shaped) flow a
# panel performs, with zero flare-edge code changes (the portal URL is a state
# file; 10.0.2.2 is slirp's host alias).
#
# FAILS CLOSED on every missing prerequisite: never a soft skip.
#
# Usage: portal-scenario.sh <zImage-virt>
# Env:   FLARE_EDGE  path to a flare-edge checkout (provides mock-flare-portal.py)
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/

ZIMAGE="${1:-}"
if [ -z "$ZIMAGE" ] || [ ! -f "$ZIMAGE" ]; then
  echo "FATAL: usage: $0 <zImage>: the virt.fragment kernel variant" >&2
  exit 1
fi
if [ -z "${FLARE_EDGE:-}" ] || [ ! -f "$FLARE_EDGE/tools/mock-flare-portal.py" ]; then
  echo "FATAL: FLARE_EDGE must point at a flare-edge checkout (mock-flare-portal.py not found under '${FLARE_EDGE:-}')" >&2
  exit 1
fi
[ -x "$QDIR/payload/warden-flared" ] || {
  echo "FATAL: no qemu/payload/warden-flared: build a static musl armv7 flared (see qemu/payload/README.md)" >&2
  exit 1
}
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH: see qemu/README.md" >&2
  exit 1
}

# Short-named scratch: AF_UNIX socket paths are capped at ~108 chars.
WORK="$(mktemp -d /tmp/wqp.XXXXXX)"
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

# 0. a real signed tier-1 .wfw offer (version above the image's 0.0.1), so the
#    scenario exercises desired-state -> download -> signature/hash verify, not
#    just an empty 204. Signed with the committed desk key; the payload flared
#    is a FLARED_DEV_KEY=1 build that trusts it (desk-testing only).
head -c 8388608 /dev/urandom > "$WORK/rootfs-payload.img"
FW_SIGNING_KEY_FILE="$FLARE_EDGE/tools/testdata/fw-dev-key.seed" \
  WARDEN_KERNEL_VERSION=6.18.46 WARDEN_BUILDROOT_VERSION=2025.02 \
  WARDEN_UBOOT_VERSION=2017.09 \
  bash "$FLARE_EDGE/tools/mk-wfw.sh" "$WORK/rootfs-payload.img" 1 0.0.2 "$WORK/offer.wfw"

# 1. mock portal on the host, our device pre-registered (no pairing needed:
#    the same credential-seeding shortcut fw-e2e-test.sh uses), offering the .wfw.
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
# The loop must not fall through silently: an alive-but-unresponsive mock
# would otherwise surface 420s later as an unrelated assertion timeout.
[ "$mock_ready" = 1 ] || {
  echo "FATAL: mock portal never answered on :$PORT within 10s" >&2
  tail -10 "$WORK/mock.log" >&2
  exit 1
}
echo "== mock portal on :$PORT, device $DEVICE_ID"

# 2. image seeded with the portal URL + credentials.
bash "$QDIR/mkinitramfs.sh"
# All four enrolment keys: flare::enrolment() returns None (and the report
# loop parks forever) unless flare.site is present too.
bash "$QDIR/mkimage.sh" \
  --portal-url "http://10.0.2.2:$PORT" \
  --state "flare.device_id=$DEVICE_ID" \
  --state "flare.api_key=$API_KEY" \
  --state "flare.site=qemu-devsim"

# 3. boot the VM headless (daemons run; console log to file). Random hostfwd
#    ports can collide with another process. Detect the early qemu bind
#    failure and retry with a fresh base rather than failing spuriously.
QEMU_PID=""
for _attempt in 1 2 3; do
  VMBASE=$((20000 + RANDOM % 20000))
  : > "$WORK/console.log"
  bash "$QDIR/run.sh" --kernel "$ZIMAGE" \
    --ssh-port "$VMBASE" --http-port $((VMBASE + 1)) --api-port $((VMBASE + 2)) \
    > "$WORK/console.log" 2>&1 &
  QEMU_PID=$!
  sleep 3
  if kill -0 "$QEMU_PID" 2>/dev/null; then
    break
  fi
  if grep -aq 'Could not set up host forwarding' "$WORK/console.log"; then
    echo "== hostfwd port collision on base $VMBASE, retrying"
    QEMU_PID=""
    continue
  fi
  echo "FATAL: VM died at launch:" >&2
  tail -20 "$WORK/console.log" >&2
  exit 1
done
if [ -z "$QEMU_PID" ] || ! kill -0 "$QEMU_PID" 2>/dev/null; then
  echo "FATAL: could not launch the VM after 3 port attempts" >&2
  exit 1
fi

# 4. assert: rootfs up, and the portal saw (from OUR device id) an
#    authenticated check-in, the firmware desired-state pull, and the signed
#    .wfw asset download (i.e. flared accepted the offer and fetched it; the
#    verify+stage+APPLYING that follow are a dry run without
#    WARDEN_FW_ALLOW_APPLY, exactly like fw-e2e-test.sh).
deadline=$((SECONDS + 420))
ok_boot=0 ok_report=0 ok_fw=0 ok_asset=0
while [ $SECONDS -lt $deadline ]; do
  [ $ok_boot -eq 0 ] && grep -aq 'WARDEN-QEMU-ROOTFS-OK' "$WORK/console.log" && {
    ok_boot=1; echo "== VM userspace up"; }
  grep -aq "POST /api/v1/devices/$DEVICE_ID/report -> 200" "$WORK/mock.log" && ok_report=1
  grep -aq "GET /api/v1/devices/$DEVICE_ID/firmware -> 200" "$WORK/mock.log" && ok_fw=1
  grep -aq "GET /api/v1/devices/$DEVICE_ID/firmware/assets/.* -> 200" "$WORK/mock.log" && ok_asset=1
  [ $ok_report -eq 1 ] && [ $ok_fw -eq 1 ] && [ $ok_asset -eq 1 ] && break
  grep -aq 'WARDEN-QEMU-MOUNT-FAILED' "$WORK/console.log" && {
    echo "FATAL: guest partition mount failed (bad image?):" >&2
    grep -a 'WARDEN-QEMU-MOUNT-FAILED' "$WORK/console.log" >&2
    exit 1
  }
  kill -0 "$QEMU_PID" 2>/dev/null || { echo "FATAL: VM exited early" >&2; tail -30 "$WORK/console.log" >&2; exit 1; }
  sleep 2
done

echo "== portal log (our device's requests):"
grep -a "$DEVICE_ID" "$WORK/mock.log" | tail -5 || true

fail=0
[ $ok_boot -eq 1 ] || { echo "FAIL: VM never reached WARDEN-QEMU-ROOTFS-OK"; fail=1; }
[ $ok_report -eq 1 ] || { echo "FAIL: no authenticated check-in (POST report 200) seen"; fail=1; }
[ $ok_fw -eq 1 ] || { echo "FAIL: no firmware desired-state pull (GET firmware 200) seen"; fail=1; }
[ $ok_asset -eq 1 ] || { echo "FAIL: signed .wfw asset was never downloaded"; fail=1; }
if [ $fail -eq 0 ]; then echo "PORTAL-SCENARIO-PASS"; else echo "PORTAL-SCENARIO-FAIL"; exit 1; fi
