# Overnight kernel-enablement run — results (2026-08-24 night → 08-25)

Goal ([maintainer]): port/enable **every** remaining RV1106 hardware capability on the
self-built Linux 6.18.46, open-source-first, verify on hardware, so the display's
last mile can start in the morning. Runs on warden-c8a3 (`_b` slot = our 6.18).

## ✅ Verified on hardware this run
| Driver | Evidence |
|---|---|
| **AIC8800 wifi** (M5) | wlan0 up ([device-mac]), `iw scan` found the site AP −43 dBm + others. Modules (built-in deadlocks the two-stage SDIO bring-up). |
| **TRNG** | `/dev/hwrng`, `rng_current=rockchip-rng`, real entropy. |
| **OTP/nvmem** | `rockchip-otp0` reads chip id ("MR1"). |
| **GMAC** (wired eth) | `eth0: Link is Up - 100 Mbps/Full`. |
| **SARADC** | `iio:device0` reads 2 channels (adc-keys). |
| **Audio** | card `rv1106-acodec`, `/dev/snd/pcmC0D0p`+`pcmC0D0c`. Audible test → bench. |

(These join the earlier-verified set: clk, pinctrl, GIC, timer, DMA, eMMC, uart×3,
i2c, wdt, RTC, tsadc, RGA, USB host, PWM backlight. VOP display binds — connector
is the morning task.)

## Cross-cutting fixes made this run
- **XZ compression** (`CONFIG_KERNEL_XZ`): the growing kernel overran U-Boot's
  DTB-at-0xc00000 load boundary (`Sysmem Error`, FLARE-AB fell to _a). zImage
  12.1→8.3 MB, ~4 MB headroom. See `kernel-618-btest-cycle` memory + wifi doc.
- **Module vs built-in**: wifi must be modules (SDIO two-stage bring-up races the
  mmc probe when built-in — sequential initcall deadlock). Audio/GMAC/etc. are
  built-in and fine.
- Hardened the c8a3 `_b`-test → `_a`-recover cycle (compact-hex AVB serial write +
  readback while stable; Pi golden-env fallback for bootcount→loader).

## Not done — deferred with reasons (see CAPABILITIES-AUDIT.md)
mailbox (binds via rk3368 fallback but no client to exercise), crypto-v3 (CPU
crypto extensions already cover it; 100 KB port), NPU (no open userspace),
pvtm (DVFS-only). Camera/ISP/SPI: no hardware. **No unexplored capability gap.**

## Morning (needs [maintainer] at the bench)
1. **Display connector** — VOP binds + DRM card0 + panel probes, but no connector
   link yet; needs eyes on the panel (pixels can't be verified over serial).
2. **Audible audio** — `speaker-test`/`aplay` through the acodec.
3. Wifi **boot-time auto-load** (modules currently insmod'd by hand) — a loader
   that inserts aic8800_bsp→fdrv(→btlpm) from the rootfs.

Provenance: every ported driver is GPL-2.0 kernel source (`PROVENANCE.md`).
