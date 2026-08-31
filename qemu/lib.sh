# shellcheck shell=bash
# Shared helpers for the qemu/ device-sim build scripts. Sourced, not executed.
# Callers must run under `set -euo pipefail` and define QEMU_DIR (the qemu/ dir).

BB_VER=1.31.0
BB_URL="https://busybox.net/downloads/binaries/${BB_VER}-defconfig-multiarch-musl/busybox-armv7l"

qemu_log() { printf '\033[36m== %s\033[0m\n' "$*"; }

# Fetch (or accept via $BUSYBOX) the pinned static armv7 busybox and verify it
# against qemu/busybox.sha256. FAILS CLOSED: a missing pin refuses to build,
# never silently skips verification, mirroring build/build-kernel.sh's
# tarball handling. Sets $BB to the verified binary's path.
qemu_get_busybox() {
  local sha_file="$QEMU_DIR/busybox.sha256"
  local out="${OUT:-$QEMU_DIR/out}"
  mkdir -p "$out"
  BB="${BUSYBOX:-$out/busybox-armv7l}"
  # Pin first: a missing pin refuses BEFORE downloading, same ordering as
  # build/fetch-kernel-tarball.sh.
  [ -f "$sha_file" ] || {
    echo "FATAL: no pinned sha256 for busybox (expected $sha_file): refusing to build from an unverified binary" >&2
    exit 1
  }
  if [ ! -f "$BB" ]; then
    qemu_log "downloading $BB_URL"
    curl --retry 3 --retry-delay 5 --retry-connrefused -fSL "$BB_URL" -o "$BB"
  fi
  local want got
  want="$(cat "$sha_file")"
  got="$(sha256sum "$BB" | awk '{print $1}')"
  [ "$want" = "$got" ] || { echo "busybox sha256 mismatch: want $want got $got" >&2; exit 1; }
  qemu_log "busybox sha256 verified"
}

# Stage the shared rootfs skeleton (qemu/rootfs/ + busybox) into $1.
# Requires qemu_get_busybox to have run (uses $BB).
qemu_stage_rootfs() {
  local root="$1"
  mkdir -p "$root/bin" "$root/sbin" "$root/dev" "$root/proc" "$root/sys" \
           "$root/etc" "$root/tmp" "$root/mnt" "$root/userdata" "$root/oem" \
           "$root/usr/bin" "$root/usr/share/udhcpc"
  install -m 0755 "$BB" "$root/bin/busybox"
  cp -a "$QEMU_DIR/rootfs/." "$root/"
  chmod 0755 "$root/init" "$root/sbin/init" "$root/etc/rc" \
             "$root/usr/share/udhcpc/default.script"
}

# Parse a "SIZE[@OFFSET](NAME)" blkdevparts entry list (without the "vda:"
# prefix) and invoke a callback `$1 name offset_bytes size_bytes` per entry.
qemu_each_partition() {
  local cb="$1" parts entry size_s off_s name size off
  parts="${WARDEN_BLKDEVPARTS#*:}"
  off=0
  local IFS=','
  for entry in $parts; do
    name="${entry##*(}"; name="${name%)}"
    size_s="${entry%%(*}"
    if [ "${size_s#*@}" != "$size_s" ]; then
      off_s="${size_s#*@}"; size_s="${size_s%@*}"
      off="$(qemu_to_bytes "$off_s")"
    fi
    size="$(qemu_to_bytes "$size_s")"
    "$cb" "$name" "$off" "$size"
    off=$((off + size))
  done
}

qemu_to_bytes() {
  local v="$1" n mult=1
  case "$v" in
    *K) n="${v%K}"; mult=1024 ;;
    *M) n="${v%M}"; mult=1048576 ;;
    *G) n="${v%G}"; mult=1073741824 ;;
    *)  n="$v" ;;
  esac
  case "$n" in
    ''|*[!0-9]*) echo "FATAL: bad blkdevparts size/offset token: '$v'" >&2; exit 1 ;;
  esac
  echo $((n * mult))
}
