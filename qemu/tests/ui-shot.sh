#!/usr/bin/env bash
# Display + touch scenario: boot the VM headless with virtio-gpu, wait for the
# LVGL UI (fbdev build) to start, screendump over QMP, inject an absolute
# touch tap (virtio-tablet), screendump again. Asserts the first frame is
# non-blank; reports (does not assert) whether the tap changed pixels — the
# device-level analogue of flare-edge's Xvfb/xdotool sim-test.sh.
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

PORT=$((21000 + RANDOM % 20000))
bash "$QDIR/run.sh" --kernel "$ZIMAGE" --display headless --qmp "$WORK/qmp.sock" \
  --ssh-port "$PORT" --http-port $((PORT + 1)) --api-port $((PORT + 2)) \
  > "$WORK/console.log" 2>&1 &
QEMU_PID=$!

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
sleep 8   # let LVGL render the first frames

qmp() { python3 "$HERE/qmp.py" "$WORK/qmp.sock" "$@"; }

qmp screendump "$WORK/shot1.ppm"
# Tap the "Metrics" tab: pixel (373,40) of 720x720 scaled to the QMP absolute
# range 0..32767 — switching tabs must repaint the content area.
qmp tap 16975 1820
sleep 3
qmp screendump "$WORK/shot2.ppm"

mkdir -p "$OUTDIR"
cp "$WORK/shot1.ppm" "$OUTDIR/ui-shot1.ppm"
cp "$WORK/shot2.ppm" "$OUTDIR/ui-shot2.ppm"

# Non-blank: more than one distinct pixel value in the raw PPM payload.
python3 - "$WORK/shot1.ppm" <<'EOF'
import sys
data = open(sys.argv[1], "rb").read()
# P6 header: magic, dims, maxval, then raw RGB
parts = data.split(b"\n", 3)
pixels = parts[3] if len(parts) == 4 else b""
distinct = len(set(pixels[i:i+3] for i in range(0, min(len(pixels), 3*720*720), 3)))
print(f"shot1: {len(pixels)} bytes of pixels, {distinct} distinct colors")
sys.exit(0 if distinct > 1 else 1)
EOF

if cmp -s "$WORK/shot1.ppm" "$WORK/shot2.ppm"; then
  echo "FATAL: tapping the Metrics tab did not change the frame — touch is not reaching the UI" >&2
  exit 1
fi
echo "tap on the Metrics tab repainted the frame (touch reached the UI)"
echo "UI-SHOT-PASS (screenshots in $OUTDIR/ui-shot{1,2}.ppm)"
