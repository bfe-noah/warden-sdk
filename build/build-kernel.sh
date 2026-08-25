#!/usr/bin/env bash
# Hermetic-ish kernel build for the WardenOS 86-Panel (RV1106).
#
#   fetch pristine linux-6.18.46 -> apply patches/ in order -> configure with
#   build/warden_defconfig -> build zImage + rv1106-warden.dtb.
#
# The pristine tree is the ONLY external input; the delta is patches/. A build
# from a clean checkout + a verified tarball is reproducible.
#
# Env:
#   KERNEL_TARBALL   path to a local linux-6.18.46.tar.xz (skips the download)
#   SDK_TC           dir holding the arm-rockchip830 uclibc cross toolchain bin/
#   WORK             build scratch dir (default: a mktemp under $TMPDIR)
#   JOBS             parallel make jobs (default: nproc)
#
# Requires: `python` (not python3) on PATH — the SDK quirk; the CI runner provides
# a project-local venv. Builds are SERIAL on the shared SDK box — never run two.
set -euo pipefail

KVER=6.18.46
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # build/
REPO="$(cd "$HERE/.." && pwd)"
PATCHES="$REPO/patches"
SHA_FILE="$HERE/linux-$KVER.tar.xz.sha256"
JOBS="${JOBS:-$(nproc)}"
URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-$KVER.tar.xz"

# A caller-provided WORK (e.g. CI's ${{ github.workspace }}/kbuild-out, from which
# artifacts are uploaded) is left intact; a scratch dir we mktemp'd here is our own
# (>1GB of extracted source + build output) and is removed on exit so repeated runs
# on the shared SDK box don't fill the disk.
if [ -n "${WORK:-}" ]; then
  WORK_OWNED=0
else
  WORK="$(mktemp -d "${TMPDIR:-/tmp}/warden-kbuild.XXXXXX")"
  WORK_OWNED=1
fi
trap '[ "${WORK_OWNED:-0}" = 1 ] && rm -rf "$WORK"' EXIT

log() { printf '\033[36m== %s\033[0m\n' "$*"; }

command -v python >/dev/null || { echo "need 'python' (not python3) on PATH — SDK quirk" >&2; exit 1; }

# 1. obtain + verify the pristine tarball
mkdir -p "$WORK"
TB="${KERNEL_TARBALL:-$WORK/linux-$KVER.tar.xz}"
if [ ! -f "$TB" ]; then
  log "downloading $URL"
  curl -fSL "$URL" -o "$TB"
fi
# Fail closed: a missing pin (e.g. forgotten on a KVER bump) or a KERNEL_TARBALL
# pointed at an arbitrary file must refuse to build, never silently skip the check
# — the pristine tarball is the ONLY external input and integrity is the whole point.
[ -f "$SHA_FILE" ] || {
  echo "FATAL: no pinned sha256 for linux-$KVER (expected $SHA_FILE) — refusing to build from an unverified tarball" >&2
  exit 1
}
want="$(cat "$SHA_FILE")"
got="$(sha256sum "$TB" | awk '{print $1}')"
[ "$want" = "$got" ] || { echo "tarball sha256 mismatch: want $want got $got" >&2; exit 1; }
log "tarball sha256 verified"

# 2. extract pristine
SRC="$WORK/linux-$KVER"
rm -rf "$SRC"
log "extracting pristine"
tar -C "$WORK" -xf "$TB"

# 3. apply the patch series in order
log "applying patch series"
for p in "$PATCHES"/*.patch; do
  git -C "$SRC" apply --whitespace=nowarn "$p" 2>/dev/null \
    || patch -d "$SRC" -p1 --no-backup-if-mismatch < "$p"
  echo "  applied $(basename "$p")"
done

# 4. configure
log "configuring (warden_defconfig)"
cp "$HERE/warden_defconfig" "$SRC/.config"
export ARCH=arm CROSS_COMPILE=arm-rockchip830-linux-uclibcgnueabihf-
if [ -n "${SDK_TC:-}" ]; then export PATH="$SDK_TC:$PATH"; fi
command -v "${CROSS_COMPILE}gcc" >/dev/null \
  || { echo "cross toolchain ${CROSS_COMPILE}gcc not on PATH (set SDK_TC)" >&2; exit 1; }
make -C "$SRC" ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" olddefconfig >/dev/null

# 5. build zImage + the board dtb
log "building zImage + rv1106-warden.dtb (-j$JOBS)"
make -C "$SRC" ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" -j"$JOBS" \
  zImage rockchip/rv1106-warden.dtb

Z="$SRC/arch/arm/boot/zImage"
D="$SRC/arch/arm/boot/dts/rockchip/rv1106-warden.dtb"
log "build OK"
ls -la "$Z" "$D"
echo "zImage: $Z"
echo "dtb:    $D"
