# M2 boot bring-up on warden-c8a3 (live notes)

Booting the self-built 6.18 kernel on real hardware (warden-c8a3), 2026-08-24.
The build side (zImage + dtb) is in `../rv1106-enablement/`; this is the on-target
half. **The safe-test framework and the boot-image format below are the reusable
findings: they worked on the first hardware try.**

## The safe test path (A/B slot _b, never touch _a)

c8a3 is the desk-connected bench Warden: reachable over its USB-gadget ethernet
(dev-build dropbear; address and bench credentials live in the private
deployment notes: use `scp -O`, no sftp-server), plus the serial console at
115200 and remotely switchable power. It runs an A/B
firmware (boot_a=mmcblk0p5 / boot_b=mmcblk0p6, 32 MiB each; rootfs_a/_b; AvbABData
in `misc` sector 4 / byte 2048).

The test never risks the working system:
1. Back up `boot_b` (`dd .../by-name/boot_b -> /userdata/boot_b.bak`).
2. Write the test `boot.img` to **boot_b only**. `_a` (the shipped WardenOS) is
   untouched.
3. Flip AvbABData to a **one-shot _b**: `craft_ab.py 14 0 1 15 1 0 1` -> A(prio14,
   tries0,ok1) B(prio15,tries1,ok0); write to `misc` sector 4. U-Boot's SPL then
   picks _b once, decrements tries->0, and boots it.
4. If the test kernel fails/hangs, U-Boot auto-reverts to `_a` (SPL: "slot boot
   failed, resetting" -> next boot `A/B-slot: _a, successful: 1`). A hung kernel
   just needs a Zigbee power-cycle; `_a` boots WardenOS back. **Zero manual
   recovery needed: verified across three failed attempts.**

`earlycon=uart8250,mmio32,0xff4c0000` is confirmed correct: it is in the live
5.10 cmdline, and U-Boot's own DDR probe reports `Adding bank: 0x0 - 0x10000000`
(256 MiB), so the M2 DT's memory node and earlycon are right.

## The boot-image format (this was the whole fight)

rockchip U-Boot (this build) needs a very specific `boot.img`, NOT a plain FIT:

- **External-data FIT.** `mkimage -f its -E -p 0x800`. The FDT metadata stays tiny
  (totalsize ~ 1536 B, matching boot_a); the kernel/fdt/resource **data is
  appended** after it. A normal embedded-data FIT (totalsize = whole image) is
  rejected with `FIT: No fit blob` / `No FIT image`.
- **A `resource` (multi) sub-image is mandatory.** rockchip reads the DTB via the
  `RESC:` path from a resource image containing `rk-kernel.dtb` (+ logos), built
  with `resource_tool --pack`. Without it: `Failed to load DTB, ret=-19`.
- **Sysmem sentinel load addresses.** fdt `load=0xffffff00`, kernel
  `load=entry=0xffffff01`; U-Boot's sysmem places them (it chose kernel@0x8000,
  fdt@0xc00000). Real low addresses collided -> "No fit blob".
- `CONFIG_FIT_SIGNATURE` is **off** in this U-Boot, so the image need not be
  signed. Template: `sdk/sysdrv/source/kernel/boot.its`; builder recipe:
  `sdk/project/scripts/mk-fitimage.sh` (`mkimage -E -p 0x800`).

With the correct format, U-Boot loaded my kernel + my DTB and printed my DT model
string (`Model: WardenOS 86-Panel (RV1106), M2 earlycon bring-up`), then
`Starting kernel ...`.

## Result: [x] M2 achieved, the 6.18 kernel boots on hardware

Six attempts, each auto-recovering to `_a`, then a clean boot:

```
[0.000000] Linux version 6.18.46 ... #2 SMP
[0.000000] CPU: ARMv7 Processor [410fc075]
[0.000000] OF: fdt: Machine model: WardenOS 86-Panel (RV1106), M2 earlycon bring-up
[0.000000] earlycon: uart8250 at MMIO32 0xff4c0000
[0.000000] cma: Reserved 64 MiB at 0x0c000000
[0.040693] Calibrating delay loop ... 48.00 BogoMIPS   <- arch timer up
[0.343810] pinctrl core: initialized pinctrl subsystem
[1.968703] ff4c0000.serial: ttyS2 ... is a 16550A       <- 8250 on our CRU clock
```

Two more bugs, found via the DEBUG_LL rebuild (`DEBUG_LL_UART_8250`, PHYS
0xff4c0000, shift 2, 32-bit word + `earlyprintk`, the decompressor prints
pre-MMU), then fixed:

- **DTB overrun -> `r2=0` / `invalid dtb`.** The multi_v7 zImage decompresses to
  ~20 MiB from 0x8000, overrunning the DTB at 0xc00000, so the decompressor handed
  the kernel a null DTB pointer. Fix: place the fdt high (`load=0x08000000`) in the
  FIT `.its` (see `boot5.its`). The real fix is a lean defconfig; multi_v7 is bloat.
- **`grf_ctx` NULL deref in clk-rv1106.** `rockchip_clk_register_branches(grf_ctx,...)`
  crashed because the minimal DT had no `rockchip,rv1106-grf-cru` node to set
  `grf_ctx`. Fix: add the `grf-clock-controller` child to the grf syscon (now in
  `dts/rv1106-warden-m2.dts`).

**Console baud gotcha:** `console=ttyS2,115200`. earlycon is readable at 115200
(U-Boot leaves uart2 there), but the vendor's 1.5M console rate is garbage on the
CP2102 bench adapter, so the M2 DT pins 115200 for readable bring-up; production
overrides to 1.5M.

## M3 (same session): [x] the full WardenOS runs on the 6.18 kernel

Adding the eMMC `dw_mmc` node (`mmc@ffa90000`, clocks from cru + grf_cru) was the
only change M3 needed: the mmc/ext4 drivers are already in-config. The kernel
enumerated the eMMC at HS200, mounted the ext4 rootfs, ran `/sbin/init`, and
started every WardenOS daemon. A serial root login confirms `uname -a` ->
`Linux warden-c8a3 6.18.46 armv7l`, with `warden-flared/-modbus/-mikrotik/-asic/
-ui/-flight` all running. Expected M4/M5 gaps show cleanly: the 5.10 aic8800 `.ko`
won't load (vermagic -> M5), and there's no backlight/framebuffer yet (-> M4).

**Console lesson applied:** with `console=ttyS2,115200` the whole boot is readable
on the CP2102 (the 1.5M vendor rate is garbage on it). Serial login uses the same
`c8a3_run.py` helper (dev-build bench credentials, see private deployment notes)
as the 5.10 firmware: the userspace is unchanged.

Next: M4 (VOP2 display + panel + touch), M5 (AIC8800 SDIO port + our 4 patches),
M6 (RGA/watchdog/HPMCU/USB-OTG); plus a lean defconfig + Buildroot-on-6.18 cleanup.
