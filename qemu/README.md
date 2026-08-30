# qemu/ — the WardenOS device simulator

A QEMU virtual machine that boots the real forward-ported kernel (`build/` +
`patches/`) and real userspace, so the *device* — init, daemons, networking,
OTA, watchdog — can be tested off-hardware. The third simulator in the stack,
deliberately not named "sim":

- `lvglsim` (flare-edge) — SDL desktop build of the UI. Rendering only.
- `sim/` (this repo) — register-level Rust models of RV1106 blocks behind
  driver seams.
- `qemu/` (this) — the whole machine above the kernel entry point.

## The boundary (read this before trusting a green run)

There is no RV1106 machine model in QEMU and everything below the kernel is
closed rkbin blobs plus mask ROM, so the VM **enters at `-kernel zImage`** on
`-M virt` (generic ARMv7 machine, virtio peripherals). That means:

- **Not emulated, not tested here:** BootROM, idblock/DDR-init, SPL, U-Boot,
  the real BCB-driven A/B slot selection, bootcount auto-revert. The untested
  A/B rollback chain stays untested by this tool.
- **Substituted, not modeled:** display (virtio-gpu, not VOP), input
  (virtio-tablet, not GT911), network (virtio-net, not GMAC), storage
  (virtio-blk, not eMMC). NPU/RGA/HPMCU behavior stays `sim/` territory.
- "Boots under emulation" is **not** "works on silicon". On-device claims
  still need on-device evidence; the VM narrows which claims need a panel.

## Quick start

```sh
# 1. build the QEMU kernel variant (canonical RV1106 build + virt fragment)
WORK=$HOME/kbuild-out CROSS_COMPILE=arm-linux-gnueabihf- \
  WARDEN_KCONFIG_FRAGMENT=qemu/configs/virt.fragment bash build/build-kernel.sh

# 2. build the initramfs (sha256-pinned static busybox + qemu/rootfs/)
bash qemu/mkinitramfs.sh

# 3. smoke it
bash qemu/tests/boot-smoke.sh $HOME/kbuild-out/linux-6.18.46/arch/arm/boot/zImage
```

Host requirements: `qemu-system-arm` (Debian 13 ships QEMU 10), `curl`, `cpio`,
`gcc-arm-linux-gnueabihf` for the kernel build.

Status: bringup — boot smoke only. Disk/network harness, RS485 bridge to
`sim/`, portal scenarios, and display/touch land in later phases (see
`docs/decisions/0006-qemu-device-sim.md` once written).
