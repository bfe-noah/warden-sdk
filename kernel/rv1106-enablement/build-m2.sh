#!/usr/bin/env bash
# build-m2.sh — reproducible M2 (earlycon) build of the RV1106 → 6.18 forward-port.
#
# Produces: a zImage with our ported SoC drivers, and rv1106-warden-m2.dtb.
# Boot is a separate on-hardware step (see PORT-STATUS.md "M2 boot").
#
# Usage:  KTREE=/path/to/linux-6.18.46 SDK_TC=/path/to/toolchain/bin ./build-m2.sh
# Defaults match this workspace.
set -euo pipefail

FE="${FE:-/home/noah/projects/scada/flare-edge}"
KTREE="${KTREE:-$FE/research/linux-6.18.46}"
SDK_TC="${SDK_TC:-$FE/sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# The SDK toolchain bakes absolute paths and needs `python` (not python3) + a
# space-free PATH (see the flare-edge-sdk-path-compat note).
export PATH="$SDK_TC:/usr/bin:/bin"
export ARCH=arm
export CROSS_COMPILE=arm-rockchip830-linux-uclibcgnueabihf-

[ -d "$KTREE" ] || { echo "kernel tree not found: $KTREE" >&2; exit 1; }
command -v python >/dev/null || { echo "need 'python' on PATH (SDK quirk)" >&2; exit 1; }

# Stage our board DT into the tree if it drifted from the capture.
install -m664 "$HERE/dts/rv1106-warden-m2.dts" \
    "$KTREE/arch/arm/boot/dts/rockchip/rv1106-warden-m2.dts"
grep -q 'rv1106-warden-m2.dtb' "$KTREE/arch/arm/boot/dts/rockchip/Makefile" \
    || sed -i 's/\trk3288-vyasa.dtb/\trk3288-vyasa.dtb \\\n\trv1106-warden-m2.dtb/' \
       "$KTREE/arch/arm/boot/dts/rockchip/Makefile"

cd "$KTREE"
make ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" multi_v7_defconfig
./scripts/kconfig/merge_config.sh -m .config "$HERE/configs/m2-earlycon.fragment"
make ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" olddefconfig
make ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" -j"$(nproc)" zImage
make ARCH=arm CROSS_COMPILE="$CROSS_COMPILE" rockchip/rv1106-warden-m2.dtb

echo
echo "== M2 build OK =="
ls -la arch/arm/boot/zImage arch/arm/boot/dts/rockchip/rv1106-warden-m2.dtb
