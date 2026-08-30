#!/usr/bin/env bash
# Build the VM's virtio disk image carrying the device's canonical 12-partition
# A/B layout (qemu/blkdevparts.conf — the same string U-Boot and Linux parse on
# hardware; there is no MBR/GPT). Every partition is placed at the exact offset
# the cmdline string declares; rootfs_a/rootfs_b/oem_a/oem_b/userdata get ext4,
# the boot-chain partitions (env/idblock/uboot/misc/boot_a/boot_b/recovery)
# stay zeroed — the VM enters at -kernel and never reads them.
#
# Built entirely UNPRIVILEGED: per-partition mkfs.ext4 -d (no loop mounts, no
# sudo), then dd'd into a sparse raw image.
#
# Usage: mkimage.sh [--portal-url URL] [--state KEY=VALUE]... [--fw-version V]
# Env:
#   BUSYBOX   path to a local busybox binary (skips the download; still verified)
#   OUT       output dir (default: qemu/out); image at $OUT/disk.img
set -euo pipefail

QEMU_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=qemu/lib.sh disable=SC1091
. "$QEMU_DIR/lib.sh"
# shellcheck source=qemu/blkdevparts.conf disable=SC1091
. "$QEMU_DIR/blkdevparts.conf"
OUT="${OUT:-$QEMU_DIR/out}"
# mkfs.ext4 lives in sbin, which user shells on Debian don't have on PATH.
PATH="$PATH:/usr/sbin:/sbin"

PORTAL_URL=""
STATE_KV=()
FW_VERSION="0.0.1"
while [ $# -gt 0 ]; do
  case "$1" in
    --portal-url) PORTAL_URL="${2:?--portal-url needs a value}"; shift 2 ;;
    --state)      STATE_KV+=("${2:?--state needs KEY=VALUE}"); shift 2 ;;
    --fw-version) FW_VERSION="${2:?--fw-version needs a value}"; shift 2 ;;
    *) echo "FATAL: unknown argument '$1' (usage: mkimage.sh [--portal-url URL] [--state KEY=VALUE]... [--fw-version V])" >&2; exit 1 ;;
  esac
done

qemu_get_busybox

SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/warden-qemu-image.XXXXXX")"
trap 'rm -rf "$SCRATCH"' EXIT

# Stage the rootfs tree once (skeleton + payload into /usr/bin), reused for
# both slots so A and B start byte-identical, like a factory flash.
ROOT="$SCRATCH/root"
qemu_stage_rootfs "$ROOT"
for p in "$QEMU_DIR"/payload/*; do
  [ -f "$p" ] || continue
  case "$(basename "$p")" in README.md) continue ;; esac
  install -m 0755 "$p" "$ROOT/usr/bin/$(basename "$p")"
done

# Firmware version stamp — same path the device build writes; flared reads its
# running version here (downgrade rules key off it).
printf '%s\n' "$FW_VERSION" > "$ROOT/etc/warden-firmware-version"

# Seed persistent state (flared: one file per key under /userdata/warden).
# Newline-terminated, matching how flare-edge's fw-e2e-test.sh seeds the store.
UDATA="$SCRATCH/userdata"
mkdir -p "$UDATA/warden"
[ -n "$PORTAL_URL" ] && printf '%s\n' "$PORTAL_URL" > "$UDATA/warden/flare.url"
for kv in ${STATE_KV[@]+"${STATE_KV[@]}"}; do
  printf '%s\n' "${kv#*=}" > "$UDATA/warden/${kv%%=*}"
done

mkdir -p "$SCRATCH/empty"

# mkfs an ext4 partition image of exactly $2 bytes from staged dir $1.
mkfs_part() {
  local stage="$1" bytes="$2" img="$3"
  rm -f "$img"
  truncate -s "$bytes" "$img"
  mkfs.ext4 -F -q -d "$stage" "$img"
}

DISK="$OUT/disk.img"
rm -f "$DISK"

place_partition() {
  local name="$1" off="$2" size="$3" stage=""
  case "$name" in
    rootfs_a|rootfs_b) stage="$ROOT" ;;
    userdata)          stage="$UDATA" ;;
    oem_a|oem_b)       stage="$SCRATCH/empty" ;;
    *)                 stage="" ;;   # boot-chain partition: left zeroed
  esac
  # dd in 4K blocks — every offset in the canonical layout is 4K-aligned;
  # assert rather than assume, a misaligned write would corrupt a neighbor.
  if [ $((off % 4096)) -ne 0 ] || [ $((size % 4096)) -ne 0 ]; then
    echo "FATAL: partition $name not 4K-aligned (off=$off size=$size)" >&2
    exit 1
  fi
  DISK_END=$((off + size))
  [ -z "$stage" ] && return 0
  local img="$SCRATCH/$name.img"
  mkfs_part "$stage" "$size" "$img"
  dd if="$img" of="$DISK" bs=4096 seek=$((off / 4096)) \
     conv=notrunc,sparse status=none
  qemu_log "  $name: ext4, $((size / 1048576))M @ $off"
}

DISK_END=0
qemu_log "building $DISK ($WARDEN_BLKDEVPARTS)"
truncate -s 0 "$DISK"
qemu_each_partition place_partition
truncate -s "$DISK_END" "$DISK"
qemu_log "disk image: $DISK ($(du -h "$DISK" | cut -f1) used, $((DISK_END / 1048576))M apparent)"
