# AIC8800 wifi — VERIFIED on warden-c8a3 (self-built 6.18.46), 2026-08-25

**Result: wifi works end-to-end on our self-built Linux 6.18.46.** Modules built
from the ported source (vermagic `6.18.46 SMP mod_unload ARMv7 p2v8`), loaded on
the panel, downloaded firmware to the AIC8800DC, created `wlan0`, and completed a
live RF scan.

## Evidence (serial console, _b slot = our 6.18 kernel)
- `insmod aic8800_bsp.ko`  → firmware download OK: `aicwf_patch_config_8800dc done`,
  `Start app: 00120000`, BSP_RC=0.
- `insmod aic8800_fdrv.ko` → `ieee80211 phy0: HT supp 1, VHT supp 1, HE supp 1`,
  FDRV_RC=0.
- `wlan0: <BROADCAST,MULTICAST,UP,LOWER_UP> ... link/ether [device-mac]`
- `iw dev wlan0 scan` found real APs:
  - **the site AP  [bssid]  2412 MHz  −43 dBm**
  - a neighboring guest AP  [bssid]  2412 MHz  −73 dBm
  - +several more, correct signal strengths → RF path fully functional.

## Why MODULES, not built-in (=y)
Built-in device_initcalls run BEFORE the dw_mmc/SDIO controller probes. Initcalls
are sequential: `aicbsp_init` blocked 3.3–7.5 s doing the eager chip bring-up, and
the mmc controller only probed at 7.9 s (SDIO card at 8.2 s) — AFTER aicbsp had
already given up (`aicsdio.c:597` 2 s `down_timeout` → `sdio_unregister_driver`).
Extending the timeout can't help (aicbsp blocks the very mmc probe that would
enumerate the card — a deadlock). Loaded as modules AFTER boot (mmc up, card at
4 s), `insmod aic8800_bsp` registers the SDIO driver against an already-present
card → probe fires immediately → firmware download → fdrv → wlan0. This is the
vendor-proven flow.

## Kernel-size fix (needed to boot the wifi kernel at all)
The wifi kernel grew the gzip zImage to 12.12 MB; rockchip U-Boot loads the kernel
blob at 0x8000 and relocates the DTB to 0xc00000 (12 MB), so a >~11.95 MB zImage
overruns the FDT at U-Boot load time (`Sysmem Error: KERNEL overlap with FDT`) and
FLARE-AB falls back to _a. Switched `CONFIG_KERNEL_GZIP` → `CONFIG_KERNEL_XZ`:
zImage 12.12 MB → 8.15 MB (module build), ~4 MB headroom under the FDT. Also the
right call for a firmware kernel. (Uncompressed Image is ~30 MB; the ARM
decompressor relocates the FDT at runtime, so only the U-Boot LOAD-time overlap
mattered — proven by the kernel booting once the zImage fit.)

## Boot-time auto-load (follow-up, deployment layer — not the kernel port)
Modules were insmod'd manually for this verify. Production auto-load needs a
loader that inserts `aic8800_bsp.ko` → `aic8800_fdrv.ko` (→ `aic8800_btlpm.ko`) in
order from wherever they're staged; the vendor `insmod_wifi.sh` references a
different variant (`aic_load_fw.ko`/`bcmdhd.ko`, absent here). Track in the rootfs.
