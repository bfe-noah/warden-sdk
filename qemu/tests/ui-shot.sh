#!/usr/bin/env bash
# Display + touch scenario: boot the VM headless with virtio-gpu, wait for the
# LVGL UI (fbdev build) to render a real frame, then inject an absolute touch
# tap on the Metrics tab (virtio-tablet) and ASSERT the frame changed — the
# device-level analogue of flare-edge's Xvfb/xdotool sim-test.sh. Readiness is
# polled from screendumps on bounded deadlines, never guessed with fixed
# sleeps: TCG renders CPU-bound and a loaded host can be arbitrarily slow.
#
# FAILS CLOSED on missing prerequisites.
#
# Usage: ui-shot.sh <zImage-virt> [out-dir]
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/

ZIMAGE="${1:-}"
OUTDIR="${2:-$QDIR/out}"
if [ -z "$ZIMAGE" ] || [ ! -f "$ZIMAGE" ]; then
  echo "FATAL: usage: $0 <zImage> [out-dir] — the virt.fragment kernel variant" >&2
  exit 1
fi
[ -x "$QDIR/payload/warden-ui" ] || {
  echo "FATAL: no qemu/payload/warden-ui — build it with flare-edge tools/build-ui-vm.sh" >&2
  exit 1
}
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH — see qemu/README.md" >&2
  exit 1
}

# Short-named scratch: AF_UNIX socket paths are capped at ~108 chars.
WORK="$(mktemp -d /tmp/wqu.XXXXXX)"
QEMU_PID=""
cleanup() {
  if [ -n "$QEMU_PID" ]; then kill "$QEMU_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

bash "$QDIR/mkinitramfs.sh"
bash "$QDIR/mkimage.sh"

# Random hostfwd ports can collide — detect qemu's early bind failure and
# retry with a fresh base rather than failing spuriously.
for _attempt in 1 2 3; do
  PORT=$((21000 + RANDOM % 20000))
  : > "$WORK/console.log"
  bash "$QDIR/run.sh" --kernel "$ZIMAGE" --display headless --qmp "$WORK/qmp.sock" \
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
  echo "FATAL: VM died at launch:" >&2
  tail -20 "$WORK/console.log" >&2
  exit 1
done
if [ -z "$QEMU_PID" ] || ! kill -0 "$QEMU_PID" 2>/dev/null; then
  echo "FATAL: could not launch the VM after 3 port attempts" >&2
  exit 1
fi

deadline=$((SECONDS + 120))
while [ $SECONDS -lt $deadline ]; do
  grep -aq 'init: starting warden-ui' "$WORK/console.log" && break
  kill -0 "$QEMU_PID" 2>/dev/null || { echo "FATAL: VM exited early" >&2; tail -25 "$WORK/console.log" >&2; exit 1; }
  sleep 2
done
grep -aq 'init: starting warden-ui' "$WORK/console.log" || {
  echo "FATAL: warden-ui never started (no fb0? wrong kernel?)" >&2
  tail -25 "$WORK/console.log" >&2
  exit 1
}

qmp() { python3 "$HERE/qmp.py" "$WORK/qmp.sock" "$@"; }

# Frame is "real" once it has more than a handful of distinct colors (a blank
# or console-only frame has very few).
frame_rendered() { # $1 = ppm path
  python3 - "$1" <<'EOF'
import sys
data = open(sys.argv[1], "rb").read()
parts = data.split(b"\n", 3)               # P6 header: magic, dims, maxval, raw RGB
pixels = parts[3] if len(parts) == 4 else b""
distinct = len(set(pixels[i:i+3] for i in range(0, min(len(pixels), 3*720*720), 3)))
print(f"{sys.argv[1]}: {len(pixels)} bytes, {distinct} distinct colors")
sys.exit(0 if distinct > 16 else 1)
EOF
}

# The VM can die mid-poll (OOM, crash): check liveness before every QMP
# call so the failure is OUR message + console evidence, not a python
# traceback — and preserve the console log before the trap removes $WORK.
vm_alive_or_die() {
  kill -0 "$QEMU_PID" 2>/dev/null && return 0
  echo "FATAL: VM exited during the screendump poll" >&2
  tail -25 "$WORK/console.log" >&2
  mkdir -p "$OUTDIR"; cp "$WORK/console.log" "$OUTDIR/ui-shot-console.log" || true
  exit 1
}

# Poll for the first rendered frame (bounded, no guessed sleep).
rendered=0
deadline=$((SECONDS + 90))
while [ $SECONDS -lt $deadline ]; do
  vm_alive_or_die
  qmp screendump "$WORK/shot1.ppm"
  if frame_rendered "$WORK/shot1.ppm"; then rendered=1; break; fi
  sleep 3
done
[ "$rendered" = 1 ] || {
  echo "FATAL: UI never rendered a non-blank frame within 90s" >&2
  mkdir -p "$OUTDIR"; cp "$WORK/console.log" "$OUTDIR/ui-shot-console.log" || true
  exit 1
}

# Tap the "Metrics" tab: pixel (373,40) of 720x720 scaled to the QMP absolute
# range 0..32767 — switching tabs must repaint the content area. Poll for the
# repaint rather than guessing a delay.
vm_alive_or_die
qmp tap 16975 1820
changed=0
# 90s, matching the first-frame budget: TCG repaints are CPU-bound and a
# contended CI runner can be arbitrarily slower than this dev box (same
# margin reasoning as the rs485 test-gap widening).
deadline=$((SECONDS + 90))
while [ $SECONDS -lt $deadline ]; do
  sleep 2
  vm_alive_or_die
  qmp screendump "$WORK/shot2.ppm"
  if ! cmp -s "$WORK/shot1.ppm" "$WORK/shot2.ppm"; then changed=1; break; fi
done

mkdir -p "$OUTDIR"
cp "$WORK/shot1.ppm" "$OUTDIR/ui-shot1.ppm"
cp "$WORK/shot2.ppm" "$OUTDIR/ui-shot2.ppm" 2>/dev/null || true

[ "$changed" = 1 ] || {
  echo "FATAL: tapping the Metrics tab did not change the frame within 90s — touch is not reaching the UI" >&2
  cp "$WORK/console.log" "$OUTDIR/ui-shot-console.log" || true
  exit 1
}
echo "tap on the Metrics tab repainted the frame (touch reached the UI)"
echo "UI-SHOT-PASS (screenshots in $OUTDIR/ui-shot{1,2}.ppm)"
