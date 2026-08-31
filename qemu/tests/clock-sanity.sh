#!/usr/bin/env bash
# Clock-sanity scenario (issue #3 regression guard): boot the VM, run the
# musl-static clockprobe in the guest, and assert the vDSO monotonic RATE
# matches the kernel's /proc/uptime within 1%. Under QEMU -M virt this passes
# on the current kernel (measured 0.99963) — a regression here means the
# generic vDSO path broke. The RV1106 *board* leg of issue #3 is a separate,
# bench-only measurement; this scenario cannot see board-specific CNTFRQ or
# CNTVOFF misprogramming.
#
# FAILS CLOSED on missing prerequisites.
#
# Usage: clock-sanity.sh <zImage>
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # qemu/tests/
QDIR="$(cd "$HERE/.." && pwd)"                           # qemu/

ZIMAGE="${1:-}"
if [ -z "$ZIMAGE" ] || [ ! -f "$ZIMAGE" ]; then
  echo "FATAL: usage: $0 <zImage>" >&2
  exit 1
fi
command -v qemu-system-arm >/dev/null || {
  echo "FATAL: qemu-system-arm not on PATH — see qemu/README.md" >&2
  exit 1
}
command -v arm-linux-gnueabihf-gcc >/dev/null || {
  echo "FATAL: arm-linux-gnueabihf-gcc needed to cross-build the probe" >&2
  exit 1
}

# Build the probe for the guest (static musl armv7) and stage it as payload.
( cd "$HERE/clockprobe" && \
  CARGO_TARGET_ARMV7_UNKNOWN_LINUX_MUSLEABIHF_LINKER=arm-linux-gnueabihf-gcc \
  cargo build -q --release --target armv7-unknown-linux-musleabihf )
install -m 0755 "$HERE/clockprobe/target/armv7-unknown-linux-musleabihf/release/clockprobe" \
  "$QDIR/payload/clockprobe"

WORK="$(mktemp -d /tmp/wqc.XXXXXX)"
QEMU_PID=""
cleanup() {
  if [ -n "$QEMU_PID" ]; then kill "$QEMU_PID" 2>/dev/null || true; fi
  rm -rf "$WORK"
}
trap cleanup EXIT

bash "$QDIR/mkinitramfs.sh"
bash "$QDIR/mkimage.sh"

for _attempt in 1 2 3; do
  PORT=$((22000 + RANDOM % 20000))
  : > "$WORK/console.log"
  { sleep 40; printf '/usr/bin/clockprobe\n'; sleep 16; printf 'poweroff -f\n'; sleep 8; } | \
    bash "$QDIR/run.sh" --kernel "$ZIMAGE" --shell \
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

wait "$QEMU_PID" || true
grep -a 'CLOCKPROBE' "$WORK/console.log" || {
  echo "FATAL: probe never ran; console tail:" >&2
  tail -25 "$WORK/console.log" >&2
  exit 1
}

python3 - "$WORK/console.log" <<'EOF'
import re, sys
text = open(sys.argv[1], errors="replace").read()
m = re.search(r"ratio_mono=([0-9.]+) ratio_raw=([0-9.]+)", text)
if not m:
    sys.exit("FATAL: no ratio line in console output")
mono, raw = float(m.group(1)), float(m.group(2))
ok = abs(mono - 1.0) < 0.01 and abs(raw - 1.0) < 0.01
print(f"clock-sanity: ratio_mono={mono} ratio_raw={raw} -> {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
EOF
echo "CLOCK-SANITY-PASS"
