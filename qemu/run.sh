#!/usr/bin/env bash
# Launch the WardenOS device VM (qemu-system-arm -M virt, single Cortex-A7,
# 256M: the RV1106G3's shape). See qemu/README.md for what this does and does
# not emulate.
#
# Usage: run.sh --kernel <zImage> [options] [-- <extra qemu args>]
#   --kernel PATH      zImage (canonical or virt.fragment variant)
#   --initrd PATH      initramfs (default: qemu/out/initramfs.cpio.gz)
#   --disk PATH        virtio disk image from mkimage.sh (default: qemu/out/disk.img
#                      when present; pass --no-disk for a diskless initramfs boot)
#   --no-disk          boot without a disk (initramfs shell/smoke behavior)
#   --slot _a|_b       rootfs slot to boot (default _a)
#   --rtc DATE         guest RTC base, e.g. 2021-01-01: reproduces the no-RTC
#                      "device boots believing 2021" incident class
#   --rs485 SOCK       unix socket chardev for the RS485/Modbus bridge
#                      (pci-serial: needs the virt.fragment kernel)
#   --watchdog         add i6300esb watchdog, reset on expiry (fragment kernel)
#   --qmp SOCK         QMP unix socket (screendump, input-send-event, quit)
#   --display MODE     off (default, -nographic) | on (gtk window) | headless
#                      (virtio-gpu without a window; screendump via --qmp)
#   --ssh-port N       hostfwd 127.0.0.1:N -> guest :22   (default 2222; 0 disables)
#   --http-port N      hostfwd 127.0.0.1:N -> guest :80   (default 8080; 0 disables)
#   --api-port N       hostfwd 127.0.0.1:N -> guest :28443 (default 28443; 0 disables)
#   --shell            interactive shell in the guest instead of daemon hold
#   --allow-apply      let flared ACTUALLY apply OTA firmware (writes rootfs_b
#                      inside disk.img, safe in the VM, never the default)
set -euo pipefail

QEMU_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=qemu/blkdevparts.conf disable=SC1091
. "$QEMU_DIR/blkdevparts.conf"
OUT="${OUT:-$QEMU_DIR/out}"

KERNEL="" INITRD="$OUT/initramfs.cpio.gz" DISK="" NO_DISK=0 SLOT="_a"
RTC="" RS485="" WATCHDOG=0 QMP="" DISPLAY_MODE="off" SHELL_FLAG=0 ALLOW_APPLY=0
SSH_PORT=2222 HTTP_PORT=8080 API_PORT=28443
EXTRA=()

while [ $# -gt 0 ]; do
  case "$1" in
    --kernel)    KERNEL="${2:?}"; shift 2 ;;
    --initrd)    INITRD="${2:?}"; shift 2 ;;
    --disk)      DISK="${2:?}"; shift 2 ;;
    --no-disk)   NO_DISK=1; shift ;;
    --slot)      SLOT="${2:?}"; shift 2 ;;
    --rtc)       RTC="${2:?}"; shift 2 ;;
    --rs485)     RS485="${2:?}"; shift 2 ;;
    --watchdog)  WATCHDOG=1; shift ;;
    --qmp)       QMP="${2:?}"; shift 2 ;;
    --display)   DISPLAY_MODE="${2:?}"; shift 2 ;;
    --ssh-port)  SSH_PORT="${2:?}"; shift 2 ;;
    --http-port) HTTP_PORT="${2:?}"; shift 2 ;;
    --api-port)  API_PORT="${2:?}"; shift 2 ;;
    --shell)     SHELL_FLAG=1; shift ;;
    --allow-apply) ALLOW_APPLY=1; shift ;;
    --) shift; EXTRA=("$@"); break ;;
    *) echo "FATAL: unknown argument '$1' (see header of $0)" >&2; exit 1 ;;
  esac
done

if [ -z "$KERNEL" ] || [ ! -f "$KERNEL" ]; then
  echo "FATAL: --kernel <zImage> required and must exist (got '${KERNEL:-}')" >&2
  exit 1
fi
[ -f "$INITRD" ] || {
  echo "FATAL: initramfs not found at $INITRD: run qemu/mkinitramfs.sh" >&2
  exit 1
}
case "$SLOT" in _a|_b) ;; *) echo "FATAL: --slot must be _a or _b" >&2; exit 1 ;; esac

if [ "$NO_DISK" -eq 0 ] && [ -z "$DISK" ] && [ -f "$OUT/disk.img" ]; then
  DISK="$OUT/disk.img"
fi
if [ -n "$DISK" ] && [ ! -f "$DISK" ]; then
  echo "FATAL: disk image $DISK not found: run qemu/mkimage.sh (or pass --no-disk)" >&2
  exit 1
fi

# NOTE: never add `earlyprintk`: the config's DEBUG_UART_PHYS is the RV1106's
# 0xff4c0000, which does not exist on -M virt.
APPEND="console=ttyAMA0 rdinit=/init"
# Port 0 disables a forward. A boot smoke needs no host ports and must not
# fail on a busy default port.
NETDEV="user,id=n0"
[ "$SSH_PORT" != 0 ]  && NETDEV="$NETDEV,hostfwd=tcp:127.0.0.1:${SSH_PORT}-:22"
[ "$HTTP_PORT" != 0 ] && NETDEV="$NETDEV,hostfwd=tcp:127.0.0.1:${HTTP_PORT}-:80"
[ "$API_PORT" != 0 ]  && NETDEV="$NETDEV,hostfwd=tcp:127.0.0.1:${API_PORT}-:28443"
ARGS=(
  -M "virt,highmem=off" -cpu cortex-a7 -smp 1 -m 256M
  # virtio-mmio defaults to the legacy (0.9) transport; virtio-gpu and
  # virtio-input are VERSION_1-only devices and never bind without this.
  -global "virtio-mmio.force-legacy=false"
  -kernel "$KERNEL" -initrd "$INITRD"
  -netdev "$NETDEV"
  -device "virtio-net-device,netdev=n0"
  -no-reboot
)

if [ -n "$DISK" ] && [ "$NO_DISK" -eq 0 ]; then
  APPEND="$APPEND blkdevparts=$WARDEN_BLKDEVPARTS warden.slot=$SLOT"
  ARGS+=( -drive "if=none,file=$DISK,format=raw,id=vd0"
          -device "virtio-blk-device,drive=vd0" )
fi
[ "$SHELL_FLAG" -eq 1 ] && APPEND="$APPEND warden.shell"
[ "$ALLOW_APPLY" -eq 1 ] && APPEND="$APPEND warden.fwapply"
[ -n "$RTC" ] && ARGS+=( -rtc "base=$RTC" )
[ "$WATCHDOG" -eq 1 ] && ARGS+=( -device i6300esb -action watchdog=reset )
[ -n "$RS485" ] && ARGS+=( -chardev "socket,id=rs485,path=$RS485,server=on,wait=off"
                           -device "pci-serial,chardev=rs485" )
[ -n "$QMP" ] && ARGS+=( -qmp "unix:$QMP,server=on,wait=off" )

case "$DISPLAY_MODE" in
  off)      ARGS+=( -nographic ) ;;
  on)       ARGS+=( -device "virtio-gpu-device,xres=720,yres=720"
                    -device virtio-tablet-device -serial mon:stdio ) ;;
  headless) ARGS+=( -device "virtio-gpu-device,xres=720,yres=720"
                    -device virtio-tablet-device -display none -serial mon:stdio ) ;;
  *) echo "FATAL: --display must be off|on|headless" >&2; exit 1 ;;
esac

exec qemu-system-arm "${ARGS[@]}" -append "$APPEND" ${EXTRA[@]+"${EXTRA[@]}"}
