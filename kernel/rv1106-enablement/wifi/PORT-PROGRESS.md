# AIC8800 wifi+BT port — M5 progress (2026-08-24)

Status: **DONE — zImage + rv1106-warden.dtb build clean with the driver
built in.** Not flashed/booted (parent session verifies on hardware).

## Build status

```
export PATH=".../sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin:/usr/bin:/bin"
export ARCH=arm CROSS_COMPILE=arm-rockchip830-linux-uclibcgnueabihf-
make ARCH=arm CROSS_COMPILE=$CROSS_COMPILE zImage
make ARCH=arm CROSS_COMPILE=$CROSS_COMPILE rockchip/rv1106-warden.dtb
```
Clean rebuild (all aic8800 sources touched to force recompilation): **0
errors**, 104 warnings, all benign vendor-code style (`101
-Wmissing-prototypes` on internal non-`static` helper functions that were
never prototyped in headers, `3 -Wempty-body` on an existing `if
(...);`-with-no-body debug macro expansion) — none are correctness issues
and none were introduced by version-delta fixes. `zImage is ready`;
`rv1106-warden.dtb` compiles with only the pre-existing, unrelated `&rgb`
graph-endpoint warning (already known from M4 display work, not
wifi-related). Confirmed via `nm vmlinux.unstripped` that the driver is
actually linked in, not silently dropped: `aicbsp_sdio_driver`,
`aicwf_sdio_driver`, `rwnx_cfg80211_ops` all present, and the three
`module_init()`s appear as `device_initcall`s in the correct link order —
`__initcall__kmod_aic8800_fdrv__...rwnx_mod_init` after bsp's init and
`__initcall__kmod_aic8800_btlpm__...aic_bluetooth_mod_init` last.

## What was copied/wired

Copied `flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/{aic8800_bsp,aic8800_fdrv,
aic8800_btlpm}/*.{c,h}` into `drivers/net/wireless/aic8800/` in the 6.18
tree, source-only (excluded `.mod.c`/`.o`/`Module.symvers`/etc build
artifacts left over in the vendor tree from prior out-of-tree builds).

Dropped as dead code for this board (SDIO-only, no USB/PCIe, `CONFIG_SDIO_BT=n`):
`usb_host.{c,h}`, `rwnx_pci.{c,h}`, `aicwf_usb.c`, `btsdio.c`, `aic_btsdio.c`.

**One exception the plan flagged correctly**: `rwnx_mesh.{c,h}` was
initially dropped on the same "dead code" theory but had to be **restored**
— `rwnx_tx.c`'s `NL80211_IFTYPE_MESH_POINT` switch-case block is not
preprocessor-gated, so `rwnx_mesh.h`'s types (`struct rwnx_mesh_path`,
`struct rwnx_mesh_proxy`) and prototypes are needed to compile even though
this board never creates a mesh-type interface. Restored both files
(1KB each) and added `rwnx_mesh.o` back to the fdrv `Makefile` object list.
`usb_host.c`/`rwnx_pci.c` did NOT need restoring — no build error ever
referenced them.

New in-tree Kbuild wiring (not copied from vendor, vendor Makefiles are
out-of-tree `KDIR=` external-module style and don't apply directly):
- `drivers/net/wireless/aic8800/Kconfig` — top-level `AIC_WLAN_SUPPORT`
  bool + `AIC_FW_PATH` string, `source`s the fdrv/btlpm sub-Kconfigs.
  Wired into `drivers/net/wireless/Kconfig` (new `source` line, alongside
  the existing vendor Kconfig list).
- `drivers/net/wireless/aic8800/Makefile` — `obj-y` list preserving
  bsp → fdrv → btlpm link order (per PORT-PLAN.md §4.5, since these are
  built-in `=y` now and initcall order follows link order). Wired into
  `drivers/net/wireless/Makefile` (`obj-$(CONFIG_AIC_WLAN_SUPPORT) += aic8800/`).
- `aic8800_fdrv/Kconfig`, `aic8800_btlpm/Kconfig` — vendor's `tristate`
  symbols changed to `default y` (built-in, not modular; the 6.18 rootfs
  has no working module pipeline yet).
- Three new in-tree `Makefile`s (`aic8800_bsp/`, `aic8800_fdrv/`,
  `aic8800_btlpm/`) translating the vendor's `ccflags-$(CONFIG_X) += -DX`
  pattern into fixed `-D` flags for the one build configuration this board
  actually uses (SDIO, Rockchip, no USB/PCI/mesh-in-practice, TCP-ACK
  filter, preallocated TXQ, RF test support — see each Makefile's ccflags
  for the full list, matching the vendor Makefile defaults). `CONFIG_AIC_FW_PATH`
  needs no `-D`: it's now a real Kconfig string symbol, so
  `include/generated/autoconf.h` already defines the `CONFIG_AIC_FW_PATH`
  C-string macro the driver expects (`aicsdio.c:59`, `aic_bsp_driver.h:348`).

## API-delta fixes (file → change)

### Plan-anticipated mechanical renames (applied via scoped `sed`, all call sites)
- `from_timer()` → `timer_container_of()` — 7 sites: `aicsdio.c`,
  `aicwf_sdio.c` (×3), `rwnx_main.c`, `rwnx_rx.c` (×2).
- `del_timer_sync()` → `timer_delete_sync()` — 10 sites: `aicsdio.c`,
  `aicwf_sdio.c` (×5), `rwnx_main.c` (×2, one more than the plan's count —
  `rwnx_main.c:6177` wasn't in the plan's list but grep found it),
  `rwnx_rx.c` (×2).
- `del_timer()` → `timer_delete()` — 9 sites: `aicwf_tcp_ack.c` (×4),
  `rwnx_rx.c` (×2), `aic8800_btlpm/aic8800_btlpm.c` (×3, dead — not in this
  build's object list, fixed anyway for tree hygiene), `aic8800_btlpm/lpm.c`
  (×3, same — `lpm.c` isn't compiled, `CONFIG_SUPPORT_LPM=n`).
- `netif_rx_ni()` → `netif_rx()` — 3 sites, all `rwnx_rx.c`.

### Deltas beyond the plan's list (found by the compiler, fixed against 6.18 headers)
- **`cfg80211_rx_spurious_frame()` / `cfg80211_rx_unexpected_4addr_frame()`**
  (`rwnx_rx.c`, 2 sites): gained a `link_id` param before `gfp`; passed
  `-1` ("not applicable", per the header's own doc comment — this driver
  has no MLO).
- **`cfg80211_ch_switch_notify()` / `cfg80211_ch_switch_started_notify()`**
  (`rwnx_main.c`, 2 sites): the vendor's own version-gated compat shim
  (`rwnx_compat.h`'s `HIGH_KERNEL_VERSION3`/`4` = 6.3.0) already anticipated
  *an* API change here but guessed a 6.3-era signature that's since changed
  again by 6.18 (`notify` dropped back to 3 args `(dev, chandef, link_id)`;
  `started_notify` is 5 args `(dev, chandef, link_id, count, quiet)`, no
  trailing reserved arg). Added a new `#if LINUX_VERSION_CODE >=
  KERNEL_VERSION(6, 7, 0)` tier ahead of the existing chain rather than
  edit the historical shim; `link_id = 0` (mandatory for non-MLO per the
  header).
- **`cfg80211_ops` struct-initializer type mismatches** (`rwnx_main.c`,
  7 callbacks — all gained new params, all effectively "not applicable" for
  this single-radio, non-MLO, SDIO full-MAC driver, so the new params are
  accepted and ignored):
  - `change_beacon`: now takes `struct cfg80211_ap_update *` (wraps the old
    `cfg80211_beacon_data` as `.beacon`, plus FILS/S1G/unsol fields unused
    here) — unwrapped with `&update->beacon` so `rwnx_build_bcn()` and its
    other 3 callers stay untouched.
  - `set_monitor_channel`: gained a `struct net_device *dev` param. Rather
    than thread an unused `dev` through the well-used internal 2-arg
    `rwnx_cfg80211_set_monitor_channel()` (called elsewhere with `chandef =
    NULL` to retrieve firmware-level channel state), added a thin
    `rwnx_cfg80211_set_monitor_channel_ops()` adapter and pointed the ops
    table at that instead.
  - `set_wiphy_params`: gained `int radio_idx` before `changed`.
  - `set_tx_power`: gained `int radio_idx` between `wdev` and `type`.
  - `get_tx_power`: gained `int radio_idx, unsigned int link_id` before the
    output pointer.
  - `start_radar_detection`: gained a trailing `int link_id`.
  - `tdls_mgmt`: gained `int link_id` right after `peer`.
- **`cfg80211_cac_event()`** (`rwnx_radar.c`, 2 sites): gained a trailing
  `unsigned int link_id`; passed `0` (mandatory for non-MLO per doc comment).
- **`wakeup_source_create()`/`_add()`/`_remove()`/`_destroy()`** (`rwnx_wakelock.c`):
  removed from the public API entirely (still exist internally in
  `drivers/base/power/wakeup.c` but are no longer `EXPORT_SYMBOL`'d) —
  `wakeup_source_register(dev, name)` / `wakeup_source_unregister(ws)` is
  now the only supported entry point (confirmed: `wakeup_source_register()`'s
  own implementation is literally `create()` + conditional sysfs +
  `add()`). Rewrote `rwnx_wakeup_init()`/`_deinit()` to call
  `wakeup_source_register(NULL, name)` / `wakeup_source_unregister(ws)` —
  `dev=NULL` registers an anonymous source not tied to a `struct device`,
  matching the old `wakeup_source_create()` behavior used here. (The file
  already had a correct modern `rwnx_wakeup_register()`/`_unregister()`
  pair for a different call path — this fix makes `rwnx_wakeup_init()`
  consistent with it.)
- **`MODULE_IMPORT_NS(bare_token)` → `MODULE_IMPORT_NS("string")`** — 3
  sites (`aic_bsp_driver.c`, `rwnx_platform.c`, `rwnx_main.c`), all
  importing the same non-mainline `VFS_internal_I_am_really_a_filesystem_and_am_NOT_a_driver`
  namespace token (an Android-GKI-kernel artifact — confirmed absent from
  our mainline `fs/` tree, so this is now inert modinfo metadata, not a
  real namespace gate). `MODULE_INFO()`/`MODULE_IMPORT_NS()` now require a
  quoted string per current `include/linux/module.h`.
- **`rwnx_platform.c`: unconditional `#include "rwnx_pci.h"`** — the header
  was dropped with `rwnx_pci.c` (dead code, no PCIe on this board), but this
  one `#include` wasn't behind any guard. Guarded it behind
  `AICWF_PCIE_SUPPORT` (matching the guard its only two callers,
  `rwnx_platform_{,un}register_drv()`, already use at their call sites in
  `rwnx_main.c`) and guarded the two functions' `rwnx_pci_{,un}register_drv()`
  bodies the same way, returning `0`/no-op for the SDIO-only build instead.
- **`<linux/rfkill-wlan.h>` (vendor-only, PORT-PLAN.md §3.7)** — removed
  from 3 files (`aicsdio.c`, `aicwf_sdio.c`, `rwnx_main.c`). Traced every
  real `rockchip_wifi_power()`/`_set_carddetect()` call site first: all of
  them are either already `#if 0`'d out in the vendor source
  (`aicwf_sdio.c`) or gated behind `CONFIG_PLATFORM_ROCKCHIP2` (a symbol
  this build never defines — only plain `ROCKCHIP`), so none needed
  stubbing individually; only the unconditional header `#include`s needed
  removing. The DT's `mmc-pwrseq-simple` node does the power/reset
  sequencing instead, as planned.
- **`CONFIG_RFTEST` added to `aic8800_fdrv`'s ccflags** — not itself a
  version delta, but needed: `aic_priv_cmd.c`'s RF-test-mode enum
  (`SET_TX`, `RDWR_EFUSE_*`, ~50 constants) and struct typedefs
  (`cmd_rf_settx_t` etc.) are defined inline in the file under `#ifdef
  CONFIG_RFTEST`, and later code in the same file references them
  unconditionally. Omitting `-DCONFIG_RFTEST` (as an initial conservative
  default) produced ~90 "undeclared identifier" errors; the vendor
  Makefile's own default is `CONFIG_RFTEST=y`, so restoring it matches
  vendor intent, not a workaround.

### Duplicate-symbol fixes (the biggest surprise, not in the plan)
The vendor `aic8800_bsp` and `aic8800_fdrv` were always built as two
**independent `.ko` modules**, each with its own private symbol namespace.
Built in-tree as `=y`, both link into one `vmlinux`, and it turns out
`aic8800_bsp` carries a **complete second, self-contained copy** of large
parts of `aic8800_fdrv`'s machinery — its own SDIO byte/frame transport
(`aicwf_sdio_*`, `aicwf_bus_*`, `aicwf_frame_*`, `aicwf_tx_*`,
`aicwf_rx_*`, `crc8_ponl_107`), its own message/debug-command layer
(`rwnx_send_dbg_*_req`, `rwnx_cmd_mgr_{init,deinit}`,
`rwnx_rx_handle_msg`), and a few globals (`chip_mcu_id`, `chip_sub_id`,
`aic_fw_path`, `testmode`) — used for the bsp-side firmware bring-up phase
before fdrv's own driver instance re-probes and takes over the SDIO
function for real runtime operation. **49 symbols total**, `ld` caught
every one as "multiple definition" at the final `vmlinux.o` link (the
per-subdirectory `built-in.a` build had already succeeded — this class of
error only surfaces at the whole-image link, which is worth knowing for
anyone repeating this kind of port).

Verified via `aic_bsp_export.h` (the real, narrow, intentional
`aicbsp_*`-prefixed bsp→fdrv API surface) that none of the 49 are part of
the actual cross-module contract, and via `grep -rlw` that `fdrv` has its
own **complete** definition of every one (not an `extern` pointing at
bsp's copy) — confirming these are two truly independent implementations,
not one legitimately shared. Fix: renamed all 49 symbols with an
`aicbsp_priv_` prefix, scoped strictly to `aic8800_bsp/*.{c,h}` (both
definitions and bsp-internal call sites via whole-word `sed`); `fdrv`'s
copies are untouched. One of the 49, `md5.c`'s MD5 functions, got a
different (smaller-footprint) fix: `aic8800_bsp/md5.c` and
`aic8800_fdrv/md5.c` are **byte-identical** files, so `md5.o` was simply
dropped from `aic8800_fdrv`'s object list instead of renamed — `fdrv`'s
calls resolve against `bsp`'s copy at link time since bsp links first.

## Config symbols set

```
CONFIG_AIC_WLAN_SUPPORT=y
CONFIG_AIC8800_WLAN_SUPPORT=y
CONFIG_AIC8800_BTLPM_SUPPORT=y
CONFIG_AIC_FW_PATH="/oem/usr/ko/aic8800dc_fw"
CONFIG_WIRELESS=y CONFIG_CFG80211=y CONFIG_WLAN=y      (already y pre-port)
CONFIG_MMC=y CONFIG_MMC_DW_ROCKCHIP=y CONFIG_RFKILL=y  (already y pre-port)
CONFIG_BT=y                 (was =m, flipped to =y — no module pipeline yet)
CONFIG_BT_HCIUART=y         (was =m, flipped to =y)
CONFIG_BT_HCIUART_H4=y      (already y)
CONFIG_CRYPTO_ARC4=y CONFIG_CRYPTO_CTR=y CONFIG_CRYPTO_CCM=y CONFIG_CRYPTO_AES=y
# CONFIG_MAC80211 is not set   (explicitly left off — pure cfg80211 full-MAC
                                 driver, confirmed no mac80211/ieee80211_hw
                                 symbol usage anywhere in aic8800_fdrv)
```
Ran `olddefconfig` after setting these; verified afterward that other
ported drivers' symbols (`CONFIG_CLK_RV1106`, `CONFIG_PINCTRL_ROCKCHIP`,
`CONFIG_DRM_ROCKCHIP`, `CONFIG_MMC_DW_ROCKCHIP`) are untouched.

## Device tree

`arch/arm/boot/dts/rockchip/rv1106-warden.dts`:
- New root-level `sdio_pwrseq: sdio-pwrseq` node (`mmc-pwrseq-simple`,
  `reset-gpios = <&gpio1 RK_PA2 GPIO_ACTIVE_LOW>`), transplanted verbatim
  from the vendor board DTS.
- New `&sdmmc` override: `max-frequency = <50000000>`, `bus-width = <4>`,
  `cap-sd-highspeed`, `cap-sdio-irq`, `keep-power-in-suspend`,
  `non-removable`, `rockchip,default-sample-phase = <90>`,
  `supports-sdio`, `mmc-pwrseq = <&sdio_pwrseq>`, pinctrl
  `sdmmc0_clk`/`_cmd`/`_bus4`/`_det`, `status = "okay"`.
- `&sdio` (mmc@ff9a0000) left untouched — stays `disabled`, genuinely
  unused hardware on this board.

**Note**: the `&sdmmc` node itself (`mmc@ffaa0000`) in `rv1106.dtsi` and
the `sdmmc0_*` pinctrl groups in `rv1106-pinctrl.dtsi` were **already
present** in this tree from earlier M1–M3 work (not added by this task) —
this task only added the board-DTS *enablement* (`status = "okay"` +
properties) and the new `sdio_pwrseq` node. Verified in the compiled
`.dtb` (decompiled with `dtc -I dtb -O dts`) that both nodes carry the
correct phandles/properties.

## Hardening patches

Both hardware-verified patches from the plan are **already present in the
vendor SDK source** we copied from (not merely trackable-but-unapplied) —
confirmed two ways: `patch -p5 --dry-run` on both reported "Reversed (or
previously applied) patch detected" against the freshly-copied tree, and
`grep` found their exact markers already in place:
- `0001-aic8800dc-no-zero-timeout-spin-and-ratelimit.patch`
  (`major-app-additions/sdk-patches/wifi/patches/`): `printk_ratelimited`
  and the `max_t(u32, cmd_mgr->queue_sz, 1)` clamp are both in
  `rwnx_cmds.c` as shipped.
  `future-features-2/sdk-patches/wifi/patches/0002-aic8800dc-sdio-wakeup-sleep-not-spin.patch`:
- `usleep_range(200, 400)`, the `woke` flag, and the "SDIO wakeup failed;
  chip may be wedged" message are all in `aicwf_sdio.c` as shipped.

No action needed beyond copying the source. **Known housekeeping gap
(pre-existing, not fixed by this task, flagged by the plan)**: patch 0002
is still only tracked under `future-features-2/sdk-patches/wifi/patches/`,
not this branch's own `sdk-patches/wifi/patches/` — worth closing
separately since the fix is live in the vendor tree either way.

## Constraints honored

Only touched: the wifi driver tree (`drivers/net/wireless/aic8800/`), its
Kconfig/Makefile wiring (`drivers/net/wireless/{Kconfig,Makefile}`), the
board DTS wifi nodes (`rv1106-warden.dts`, additive only), and
wifi/BT/crypto-related `.config` symbols. Did not touch any other driver,
DT node, or unrelated config symbol — spot-checked after the port that
`CONFIG_CLK_RV1106`, `CONFIG_PINCTRL_ROCKCHIP`, `CONFIG_DRM_ROCKCHIP`,
`CONFIG_MMC_DW_ROCKCHIP` are all still `=y`, and the `&rgb`/VOP-related dtc
warning is the same pre-existing one from M4 (not new). Never touched
hardware — no kflash, no reboot, no `/dev/ttyUSB2`, no Pi.

## What the parent should watch for on hardware

Per PORT-PLAN.md §6's verify sequence:
1. `dmesg`: `dwmmc_rockchip ffaa0000.mmc: ...` binding (mirrors the M3
   eMMC pattern on `ffa90000.mmc`), then an SDIO card enumerating on it
   (vendor/device ID `0xc8a1`/`0xc08d`), then `aic8800_bsp`'s probe firing.
2. **Firmware path**: `AIC_FW_PATH` defaults to `/oem/usr/ko/aic8800dc_fw`
   — a 5.10-era Buildroot `/oem` partition path. Confirm the 6.18 rootfs
   actually has firmware there (or update `CONFIG_AIC_FW_PATH` to wherever
   it landed) — watch specifically for firmware-open failures in dmesg as
   the first failure mode, before assuming a driver/DT bug.
3. `wlan0` should appear in `ip link` once `aic8800_fdrv` attaches.
4. `iw phy` should show one 2.4 GHz band, 14 channels (matches the known
   5.10 baseline — a mismatch signals a cfg80211/wiphy port bug, not a
   chip issue).
5. `iw dev wlan0 scan` and a real AP association exercise the
   wiphy-locking behavior (§3.3 of the plan) — this is the area with the
   least direct verification in this port (the driver's own ops callbacks
   need no new locking per the plan's analysis, since cfg80211 core now
   holds the wiphy mutex before calling in; the async-context notification
   calls from `rwnx_msg_rx.c` were flagged as the highest-residual-risk
   area and were NOT touched by this build-fix pass beyond the signature
   changes above — if scan/connect hang or lockdep splats appear, look
   there first).
6. Confirm the two hardening patches are actually effective under load: no
   "cmd timed-out" console flood, no CPU-pinning busy-spin on a stressed/
   wedged SDIO link (`aicwf_bustx_thr` should sleep, not spin).
7. BT: `aic8800_btlpm` should attach; `hciattach -s 1500000 /dev/ttyS1 any
   1500000 flow nosleep` should bring up an HCI device.
8. Given the scale of the bsp/fdrv duplicate-symbol rename (49 symbols),
   if anything behaves subtly wrong in the bsp-side firmware bring-up
   phase specifically (vs. fdrv's runtime path), double-check the rename
   didn't miss a cross-file reference within `aic8800_bsp/` — it was done
   with whole-word `sed` scoped to that directory and re-verified by a
   clean rebuild, but it touched every `.c`/`.h` in that subtree.
