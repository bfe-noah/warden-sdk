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
#   CROSS_COMPILE    cross-compiler prefix (default arm-rockchip830-linux-uclibcgnueabihf-;
#                    CI overrides with the generic arm-linux-gnueabihf-)
#   WORK             build scratch dir (default: a mktemp under $TMPDIR)
#   JOBS             parallel make jobs (default: nproc)
#   WARDEN_KCONFIG_FRAGMENT
#                    optional kconfig fragment merged onto warden_defconfig
#                    (qemu/configs/virt.fragment builds the QEMU -M virt variant);
#                    every fragment option is verified to have taken effect
#   WARDEN_CCACHE=1  compile through ccache (CI caches ~/.ccache)
#
# Requires: `python` (not python3) on PATH — the SDK quirk; the CI runner provides
# a project-local venv. Builds are SERIAL on the shared SDK box — never run two.
set -euo pipefail

KVER=6.18.46
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # build/
REPO="$(cd "$HERE/.." && pwd)"
PATCHES="$REPO/patches"
JOBS="${JOBS:-$(nproc)}"
# The tarball URL + sha256 pin live in fetch-kernel-tarball.sh (shared with CI).

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
# Fetch + fail-closed sha256 verification live in ONE place shared with CI
# (a missing pin or a mismatch always refuses to build).
bash "$HERE/fetch-kernel-tarball.sh" "$TB"

# 2. extract pristine
SRC="$WORK/linux-$KVER"
rm -rf "$SRC"
log "extracting pristine"
tar -C "$WORK" -xf "$TB"

# 3. apply the patch series in order (fail loudly — never echo a lie)
log "applying patch series"
for p in "$PATCHES"/*.patch; do
  if git -C "$SRC" apply --whitespace=nowarn "$p" 2>/dev/null; then
    :
  elif patch -d "$SRC" -p1 --forward --no-backup-if-mismatch < "$p" >/dev/null 2>&1; then
    :
  else
    echo "FATAL: failed to apply $(basename "$p")" >&2
    exit 1
  fi
  echo "  applied $(basename "$p")"
done

# Guard against a SILENT no-op: `git apply` run from inside another git repo's
# subdirectory ignores out-of-subdir paths and exits 0 without applying anything
# (issue #1). $WORK must therefore live OUTSIDE any git checkout. Assert that a known
# product of the series actually landed on disk, so this can never masquerade as
# success again.
SENTINEL="$SRC/arch/arm/boot/dts/rockchip/rv1106-warden.dts"
[ -f "$SENTINEL" ] || {
  echo "FATAL: patch series did not apply (missing $SENTINEL)." >&2
  echo "       Is \$WORK inside a git repo? git apply silently ignores out-of-subdir" >&2
  echo "       paths there — point WORK at a dir outside any checkout (e.g. \$RUNNER_TEMP)." >&2
  exit 1
}
log "patch series applied ($(basename "$SENTINEL") present)"

# 4. configure
log "configuring (warden_defconfig)"
cp "$HERE/warden_defconfig" "$SRC/.config"
# Optional kconfig fragment overlay (e.g. qemu/configs/virt.fragment for the
# QEMU -M virt device-sim variant). Fail closed if set but unreadable — never
# silently build the wrong kernel. Unset => the canonical RV1106 build,
# byte-identical to a build without this hook.
if [ -n "${WARDEN_KCONFIG_FRAGMENT:-}" ]; then
  [ -f "$WARDEN_KCONFIG_FRAGMENT" ] || {
    echo "FATAL: WARDEN_KCONFIG_FRAGMENT set but not a file: $WARDEN_KCONFIG_FRAGMENT" >&2
    exit 1
  }
  FRAG="$(cd "$(dirname "$WARDEN_KCONFIG_FRAGMENT")" && pwd)/$(basename "$WARDEN_KCONFIG_FRAGMENT")"
  log "merging kconfig fragment $(basename "$FRAG")"
  ( cd "$SRC" && ARCH=arm ./scripts/kconfig/merge_config.sh -m .config "$FRAG" )
fi
# CROSS_COMPILE defaults to the Luckfox SDK uclibc prefix (set SDK_TC to its bin/),
# but the kernel is freestanding, so a caller may override with a generic arm cross
# toolchain instead — e.g. CROSS_COMPILE=arm-linux-gnueabihf- (in Debian's
# gcc-arm-linux-gnueabihf), which the CI runner already has on PATH.
export ARCH=arm
export CROSS_COMPILE="${CROSS_COMPILE:-arm-rockchip830-linux-uclibcgnueabihf-}"
if [ -n "${SDK_TC:-}" ]; then export PATH="$SDK_TC:$PATH"; fi
command -v "${CROSS_COMPILE}gcc" >/dev/null \
  || { echo "cross toolchain ${CROSS_COMPILE}gcc not on PATH (set SDK_TC, or CROSS_COMPILE to one that is)" >&2; exit 1; }
make -C "$SRC" ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" olddefconfig >/dev/null

# Fragment took-effect assertion: merge_config -m only pastes text, and
# olddefconfig silently resolves any symbol whose dependencies are unmet —
# a fragment option could be dropped without a word. Verify every explicit
# request in the fragment survived into the final .config; fail loud if not.
if [ -n "${WARDEN_KCONFIG_FRAGMENT:-}" ]; then
  frag_fail=0
  while IFS= read -r line || [ -n "$line" ]; do
    case "$line" in
      CONFIG_*=*)
        grep -qxF "$line" "$SRC/.config" || {
          echo "FATAL: fragment option '$line' did not take effect (unmet Kconfig dependency?)" >&2
          frag_fail=1
        } ;;
      "# CONFIG_"*" is not set")
        # A disable succeeded if the symbol is NOT set: Kconfig writes either
        # the literal "is not set" line or (when dependencies gate the symbol
        # out) nothing at all — both are valid outcomes. Only "still =value"
        # is a failed disable. (A typo'd symbol disables nothing and is
        # harmless by construction.)
        opt="${line#\# }"; opt="${opt% is not set}"
        grep -qE "^$opt=" "$SRC/.config" && {
          echo "FATAL: fragment disabled '$opt' but it is still set in the final .config" >&2
          frag_fail=1
        } ;;
    esac
  done < "$FRAG"
  [ "$frag_fail" = 0 ] || exit 1
  log "fragment options verified in final .config"
fi

# Optional ccache (CI: cache ~/.ccache across dispatches; harmless if unset).
KCC="${CROSS_COMPILE}gcc"
if [ "${WARDEN_CCACHE:-0}" = 1 ]; then
  command -v ccache >/dev/null || { echo "FATAL: WARDEN_CCACHE=1 but ccache not installed" >&2; exit 1; }
  KCC="ccache ${CROSS_COMPILE}gcc"
fi

# 5. build zImage + the board dtb
log "building zImage + rv1106-warden.dtb (-j$JOBS)"
make -C "$SRC" ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" CC="$KCC" -j"$JOBS" \
  zImage rockchip/rv1106-warden.dtb

Z="$SRC/arch/arm/boot/zImage"
D="$SRC/arch/arm/boot/dts/rockchip/rv1106-warden.dtb"
log "build OK"
ls -la "$Z" "$D"
echo "zImage: $Z"
echo "dtb:    $D"
