# AIC8800 WiFi+BT SDIO driver — 5.10 → 6.18 port plan

Target: the M5 milestone of `../PORT-STATUS.md` / `../../docs/bringup.md` — port
the AIC8800 SDIO WiFi+BT driver onto the self-built **Linux 6.18.46** kernel that
already boots WardenOS on `warden-c8a3` (M0–M3 done; M4 display in progress). No
plan44 code (per the 2026-08-23 decision in `bringup.md`) — this is a direct
forward-port of our own vendor source, same method already proven for
clk/pinctrl/mach/mmc.

## 0. Source recommendation

**Port from our own vendor SDK source**, not from any external fork:
`flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/` (§1 below is the full map). This
is not a default-to-what-we-have choice — it's the correct one on the evidence:

- **Nothing more upstream exists.** AICSemi has no public upstream driver repo
  with a stable, discoverable URL, and there is no aic8800 code in Linux
  `staging` or any LKML patch series (mainline/staging status: **absent** —
  confirmed by a dedicated web-research pass; no counter-evidence found).
  There is nothing more "canonical" to port from than the vendor source we
  already have.
- **Our tree already carries two hardware-verified fixes newer forks don't
  have**: the `queue_sz`-zero console-flood clamp
  (`flare-edge/*/sdk-patches/wifi/patches/0001-aic8800dc-no-zero-timeout-spin-and-ratelimit.patch`)
  and the SDIO-wakeup busy-spin→sleep Tier-1 hardening (live in the vendor tree
  today per `luckfox-pico-86-panel/wifi-bluetooth-aic8800.md`, tracked as
  `future-features-2/sdk-patches/wifi/patches/0002-aic8800dc-sdio-wakeup-sleep-not-spin.patch`
  — **not yet copied into this branch's `sdk-patches/`; carry both forward into
  the ported tree, and file the housekeeping gap separately**). Starting from a
  third-party fork would mean re-discovering and re-fixing both bugs on
  hardware.
- **The vendor source already anticipates part of the delta.** `rwnx_compat.h`
  has `#if LINUX_VERSION_CODE` shims up to `KERNEL_VERSION(5, 15, 60)` — e.g.
  `rwnx_main.c`'s `net_device_ops` already switches between `.ndo_do_ioctl` and
  `.ndo_siocdevprivate` at the 5.15 boundary (rwnx_main.c:1457–1474). That
  removes one whole item from the delta checklist below at zero cost.
- **A cleaner community fork exists but is not itself a base — it's a reference.**
  `radxa-pkg/aic8800` (github.com/radxa-pkg/aic8800) is an actively maintained,
  GPL-3.0, DKMS-packaged build of the same AICSemi driver family, explicitly
  patched for kernel **6.12/6.13** on Rockchip SDIO boards (Rock 3C/5C), and is
  the only community effort found that has already absorbed the post-5.15
  cfg80211/netdev/timer churn this port needs. **Use it the same way the
  pinctrl port used the upstream RV1106 patch series — as a correctness
  oracle to diff against**, not as code we vendor: it targets AIC8800D80 SDIO/USB/PCIe
  variants generically, not our AIC8800DC + our two local hardening patches,
  and an Armbian forum thread reports open regressions on it at the 6.12+
  boundary — so treat its fixes as a second opinion on the wiphy-lock and
  timer-rename hunks, verify each independently.
- **No mainline/staging path exists to lean on instead.** Confirmed by the
  research pass: no aic8800 in `drivers/net/wireless/`, `staging/`, or any
  LKML series. This matches the driver-parity table's row (`AIC8800 wifi
  (bsp/fdrv) | out-of-tree`) and the bring-up doc's framing: "AIC8800 wifi
  (plan44 has none; ours)."

## 1. SDK source map

Vendor tree root: `flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/` (an **out-of-tree**
external-module build against `KDIR := ../../../source/kernel`, i.e. the vendor
5.10.160 tree at `flare-edge/sdk/sysdrv/source/kernel/`). Selected by
`RK_ENABLE_WIFI_CHIP=AIC8800DC` in
`flare-edge/sdk/project/cfg/BoardConfig_IPC/BoardConfig-EMMC-Buildroot-RV1106_Luckfox_Pico_86Panel-IPC.mk:138`,
dispatched by `sdk/sysdrv/drv_ko/wifi/Makefile`'s `build-sdio` target
(`ifneq ($(findstring $(RK_ENABLE_WIFI_CHIP),"AIC8800DC"),) @make -C aic8800dc/`).
`88,601` total lines across the three sub-drivers.

```
aic8800dc/
  Kconfig                  # AIC_WLAN_SUPPORT, AIC_FW_PATH (default "/oem/usr/ko/aic8800dc_fw")
                            #   sources drivers/net/wireless/aic8800/{aic8800_fdrv,aic8800_btlpm}/Kconfig
                            #   -- note this path is ALREADY the mainline drivers/net/wireless/
                            #   convention; the vendor Kconfig assumes it, easing the in-tree move.
  Makefile                 # obj-$(CONFIG_AIC8800_BTLPM_SUPPORT) += aic8800_btlpm/
                            # obj-$(CONFIG_AIC8800_WLAN_SUPPORT) += aic8800_fdrv/
                            # obj-$(CONFIG_AIC_WLAN_SUPPORT)     += aic8800_bsp/
                            # (link order = bsp, fdrv, btlpm — matches the insmod order below)
  aic8800_bsp/              # SDIO bus glue + firmware bootstrap ("bsp" = board support package)
    aic_bsp_main.c           # module_init/exit; per-chip firmware filename tables (fw_u02,
                              #   fw_8800dc_u01, fw_8800dc_u02, ...); aicbsp_probe_semaphore
    aicsdio.c                 # struct sdio_driver aicbsp_sdio_driver; SDIO_DEVICE_CLASS(WLAN)
                              #   wildcard match + internal vendor/device ID probe:
                              #   SDIO_VENDOR_ID_AIC8800DC=0xc8a1, SDIO_DEVICE_ID_AIC8800DC=0xc08d
                              #   (aicsdio.c:75,80); calls rockchip_wifi_power()/
                              #   rockchip_wifi_set_carddetect() (aicsdio.c:515-580) — see §3.7
    aic8800dc_compat.c/.h, aic8800d80_compat.c/.h   # per-chip-variant glue
    aicwf_txq_prealloc.c, md5.c, aic_bsp_driver.c/.h, aicwf_firmware_array.c/.h
  aic8800_fdrv/              # the actual cfg80211 full-MAC driver ("fdrv" = fullmac driver)
    rwnx_main.c               # struct cfg80211_ops rwnx_cfg80211_ops (line 5365); wiphy_new()
                              #   (5732), wiphy_register() (6035); struct net_device_ops
                              #   rwnx_netdev_ops (1457) / rwnx_netdev_monitor_ops (1477);
                              #   3 rtnl_lock()/rtnl_unlock() pairs (5429,6063,6097)
    rwnx_msg_rx.c             # firmware→driver event handling incl. cfg80211_connect_result()
                              #   (line 958) and cfg80211_roamed() (1006,1010)
    rwnx_cfgfile.c, rwnx_tx.c, rwnx_rx.c, rwnx_txq.c    # datapath
    aicwf_sdio.c              # the actual SDIO transport (`aicwf_sdio_driver`, line 1216) —
                              #   THIS is the file the task's "aicwf_sdio" driver name refers to;
                              #   it lives inside aic8800_fdrv/, not a separate directory. Also
                              #   calls rockchip_wifi_power()/set_carddetect() (1260-1331) and
                              #   contains the Tier-1 wakeup-sleep hardening (§0) around the
                              #   `aicwf_sdio_wakeup()` retry loop (~line 1363 in the older
                              #   line numbering cited by the wiki; grep for `usleep_range`).
    aicwf_tcp_ack.c, aicwf_rx_prealloc.c, aic_priv_cmd.c, aic_vendor.c
    rwnx_compat.h             # vendor compat shim layer, #if LINUX_VERSION_CODE guards up to
                              #   5.15.60 ONLY — everything past that (timer renames, wiphy
                              #   locking, netif_rx_ni removal) is new territory, not covered.
    usb_host.c/.h, rwnx_pci.*, rwnx_mesh.*   # dead code on this board (no USB/PCI variant used)
  aic8800_btlpm/              # Bluetooth low-power-mode / HCI wake companion module
    aic8800_btlpm.c, aic_bluetooth_main.c, lpm.c, rfkill.c   # standard rfkill_register(), no
                              #   Rockchip-specific RFKILL_RK dependency (checked, none found)
  aic8800dc_fw/                # firmware blobs, see §5
```

**Modules actually `insmod`ed on the running 5.10 panel** (from
`sdk/sysdrv/drv_ko/wifi/insmod_wifi.sh:109-124`, the `#aic8800` stanza gated on
`/proc/device-tree/model` containing `"W"` or an SDIO uevent match):
```
cfg80211.ko → libarc4.ko → ctr.ko → ccm.ko → libaes.ko → aes_generic.ko
  → aic8800_bsp.ko (sleep 0.2s) → aic8800_fdrv.ko (sleep 2s) → aic8800_btlpm.ko (sleep 0.1s)
```
`rkwifi_server` is deliberately never started (WardenOS owns `wlan0` directly —
see `insmod_wifi.sh:126-137` and `wifi-bluetooth-aic8800.md`); nothing to
replicate there. The crypto modules (arc4/ctr/ccm/aes) are dependencies of the
driver's internal key-handling, not aic8800-specific — confirm they're already
`=y`/reachable in the 6.18 config (crypto is currently listed as "⬜ batch2" in
`../DRIVER-PARITY.md`; flip alongside this work).

**DT node — correction to the task's framing.** The task description assumed
`sdio: mmc@ff9a0000`. **That is wrong for this board.** The AIC8800 is wired to
**`sdmmc: mmc@ffaa0000`** (mmc1), not `sdio: mmc@ff9a0000` (mmc2) — confirmed
directly in `rv1106g-luckfox-pico-86panel.dts:83-97` (comment literally reads
`/**********SDIO-WIFI**********/` over the `&sdmmc` node) and independently in
`luckfox-pico-86-panel/wifi-bluetooth-aic8800.md:11`/`hardware-86-panel.md:69`.
`&sdio` (mmc@ff9a0000) stays `status = "disabled"` and is unused on this board.
There is **no separate DT child node** for the aic8800 chip itself — it's
discovered purely by SDIO bus-scan + vendor/device ID match inside the driver
(`aicsdio.c`, no `of_match_table` anywhere in the tree — checked, none found);
the only DT surface is the MMC controller node plus the power-sequencing node:
```dts
sdio_pwrseq: sdio-pwrseq {                      /* rv1106g-luckfox-pico-86panel.dts:20-23 */
    compatible = "mmc-pwrseq-simple";
    reset-gpios = <&gpio1 RK_PA2 GPIO_ACTIVE_LOW>;
};
&sdmmc {                                        /* rv1106g-luckfox-pico-86panel.dts:83-97 */
    max-frequency = <50000000>;
    bus-width = <4>;
    cap-sd-highspeed;
    cap-sdio-irq;
    keep-power-in-suspend;
    non-removable;
    rockchip,default-sample-phase = <90>;
    supports-sdio;
    mmc-pwrseq = <&sdio_pwrseq>;
    pinctrl-names = "default";
    pinctrl-0 = <&sdmmc0_clk &sdmmc0_cmd &sdmmc0_bus4 &sdmmc0_det>;
    status = "okay";
};
```
The `sdmmc0_{clk,cmd,bus4,det}` pin groups are defined in
`sdk/sysdrv/source/kernel/arch/arm/boot/dts/rv1106-pinctrl.dtsi:700-731` (a
separate include from `rv1106.dtsi`) — port that block, it isn't in the DT
files the M1–M3 work already ported.

## 2. Open-source landscape (web research)

- **AICSemi upstream**: no discoverable, stable public GitHub org/repo carrying
  aic8800_bsp/fdrv/btlpm as canonical upstream — vendor releases exist only
  as SoC-vendor SDK drops (Rockchip's, in our case). Nothing more upstream to
  point at than what we have.
- **Mainline/staging**: **absent.** No aic8800 anywhere in `drivers/net/wireless/`,
  `drivers/staging/`, or any LKML/patchwork series as of this research pass.
- **Cleanest newer-kernel fork found**: `radxa-pkg/aic8800`
  (github.com/radxa-pkg/aic8800) — actively maintained, GPL-3.0, DKMS-packaged,
  patched for kernel **6.12/6.13** on Rockchip SDIO boards (Radxa Rock 3C/5C),
  covering SDIO/USB/PCIe AIC8800D80 variants. Firmware ships as a companion
  `aic8800-firmware` package into `/lib/firmware/aic8800_fw/`. Treat as a
  **reference/oracle for the API-delta hunks** (§3), not a vendoring source —
  see §0 for why. An Armbian forum thread ("AIC8800 wifi sdio module not
  working with kernel 6.12+") reports it has its own open regressions at that
  boundary, so cross-check rather than trust each hunk blindly.
- **Other community efforts** (Armbian, LuckFox's own OpenWrt branch, generic
  BananaPi/OpenWrt feeds): no aic8800 kmod bundled by default even where the DT
  wiring exists — `luckfox-pico-86-panel/alternative-bsps.md:20` documents this
  exact gap for LuckFox's own OpenWrt target (`cortexa7.mk` ships `kmod-rknpu-rockchip`
  only, no `kmod-aic8800`). Confirms there is no ready-made newer-kernel package
  to pull instead of porting.
- **plan44's OpenWrt RV1106 fork** (`flare-edge/research/plan44-openwrt/`,
  Linux 6.6, 152 RV1106 patches): checked directly — **contains no aic8800
  code at all** (`find ... -ipath '*aic8800*'` empty). Confirms
  `bringup.md`'s "AIC8800 wifi (plan44 has none; ours)" and the "no plan44
  code" decision doesn't cost us anything here — there's nothing to take.

## 3. 5.10 → 6.18 API-delta checklist

Grounded in two passes: (a) direct kernel.org/bootlin/LWN research on 5.10→6.18
API history, (b) `grep` evidence from the actual vendor source (file:line cited
below) so this is a checklist against *our* code, not a generic survey.

### 3.1 Timers — confirmed hard breaks, exact call sites found
`del_timer_sync`/`del_timer` → **`timer_delete_sync`/`timer_delete`**: renamed
in `9b13df3fb64e` (landed v6.2-rc1) as a **compat-wrapped** rename; **the
compat wrapper was removed in v6.15-rc1**, so by 6.18 the old names **do not
exist**. `from_timer()` → **`timer_container_of()`**: a real mainline rename
(~6.14–6.15 treewide timer-API cleanup; exact tag unconfirmed, verify against
the actual 6.18.46 headers already unpacked at
`flare-edge/research/linux-6.18.46/include/linux/timer.h`). Every call site in
the driver, found by direct grep (not estimated):

| API | File:line(s) |
|---|---|
| `from_timer()` | `aic8800_bsp/aicsdio.c:1442`; `aic8800_fdrv/aicwf_sdio.c:279,317,2933`; `aic8800_fdrv/rwnx_main.c:1724`; `aic8800_fdrv/rwnx_rx.c:1751,2027` |
| `del_timer_sync()` | `aic8800_bsp/aicsdio.c:1677`; `aic8800_fdrv/aicwf_sdio.c:1048,1054,1297,1303,3152`; `aic8800_fdrv/rwnx_main.c:2181`; `aic8800_fdrv/rwnx_rx.c:1501,1526` |
| `del_timer()` (non-sync) | `aic8800_fdrv/aicwf_tcp_ack.c:108,367,401,462`; `aic8800_fdrv/rwnx_rx.c:1921,2607`; `aic8800_btlpm/aic8800_btlpm.c:580,603,951`; `aic8800_btlpm/lpm.c:558,581,935` |
| `setup_timer()` (pre-4.14 dead branch) | `aic8800_fdrv/aicwf_tcp_ack.c:84` — already `#if LINUX_VERSION_CODE < KERNEL_VERSION(4,14,0)` guarded against a live `timer_setup()` branch; **delete the dead `#if` branch**, don't port it |
| `timer_setup()` calls (unaffected, just listed for completeness) | `aicsdio.c:2059`; `aicwf_sdio.c:3564,3589,3590`; `rwnx_main.c:1841,6118`; `rwnx_rx.c:1433,2510`; `aicwf_tcp_ack.c:87`; both btlpm files:1104/1054 |

Mechanical fix: sed-rename `from_timer`→`timer_container_of`, `del_timer_sync`→
`timer_delete_sync`, `del_timer`→`timer_delete` across the ~20 call sites above.
`timer_setup()` itself is unchanged.

### 3.2 netdev
- **`ndo_do_ioctl` removal (~6.7-era, split into `ndo_eth_ioctl`/`ndo_siocdevprivate`
  around 5.14/5.15)**: **already handled** by the vendor. `rwnx_main.c:1457-1474`
  already has `#if LINUX_VERSION_CODE < KERNEL_VERSION(5, 15, 0)` /
  `#else .ndo_siocdevprivate = rwnx_do_ioctl,` for both `rwnx_netdev_ops` and
  `rwnx_netdev_monitor_ops`. Zero work needed — the guard picks the 6.18-correct
  member automatically. **Verify only** that `rwnx_do_ioctl`'s body (line 1368)
  still compiles against the `ndo_siocdevprivate` signature
  (`int (*)(struct net_device *, struct ifreq *, void __user *, int)` vs the
  old ioctl signature) — check on the actual 6.18.46 headers.
- **`netif_rx_ni()` removed (5.18, merged into plain `netif_rx()`, safe from any
  context)**: three call sites, **not** version-guarded: `rwnx_rx.c:404,598,1651`.
  Mechanical fix: `netif_rx_ni(rx_skb)` → `netif_rx(rx_skb)`.
- **`netif_napi_add()` weight arg dropped (~6.1)**: **not used** anywhere in
  this driver (checked, no `netif_napi_add`/NAPI in the tree — the driver does
  its own kthread-based RX processing, not NAPI polling). No action.

### 3.3 cfg80211 — the hard, non-mechanical part
**Verified directly against `flare-edge/research/linux-6.18.46/include/net/cfg80211.h`**
(the actual target tree, not a guess from release notes):
- **`cfg80211_connect_result()`**: **confirmed unchanged and safe.** In 6.18.46
  it's a `static inline` that just forwards to `cfg80211_connect_bss()`
  (`cfg80211.h:8654-8661`), with the exact same 8-argument signature the driver
  already calls at `rwnx_msg_rx.c:958`. `cfg80211_roamed()` — also **confirmed
  unchanged**: 6.18.46 signature is
  `cfg80211_roamed(struct net_device *dev, struct cfg80211_roam_info *info, gfp_t gfp)`
  (`cfg80211.h:8748`), matching the driver's call at `rwnx_msg_rx.c:1006,1010`
  (`cfg80211_roamed(dev, &info, GFP_ATOMIC)` against a local
  `struct cfg80211_roam_info info`). **No code change required here**; migrating to
  `cfg80211_connect_bss()` directly is optional cleanup, not a requirement.
- **`wiphy_new()`/`wiphy_new_nm()`**: **confirmed unchanged.**
  `wiphy_new(const struct cfg80211_ops *ops, int sizeof_priv)`
  (`cfg80211.h:6250-6253`, inline wrapper over `wiphy_new_nm(ops, sizeof_priv, NULL)`)
  — matches `rwnx_main.c:5732`'s `wiphy_new(&rwnx_cfg80211_ops, sizeof(struct rwnx_hw))`
  exactly. No signature-driven work needed.
- **Wiphy locking overhaul — real, confirmed present, and now precisely scoped
  (not a guess).** `cfg80211.h` (~6266, 6325-6362) confirms `wiphy_lock()`/
  `wiphy_unlock()`/`lockdep_assert_wiphy()` and a `struct wiphy_work` deferred-work
  mechanism all exist in 6.18.46, and — the load-bearing sentence, directly
  from the `wiphy_lock()` doc comment — **"When cfg80211 ops are called, the
  wiphy is already locked."** That means:
  - The driver's `cfg80211_ops` callbacks themselves (`rwnx_cfg80211_scan`,
    `_connect`, `_disconnect`, `_add_key`, `_mgmt_tx`, etc., `rwnx_main.c:5365-5382`)
    need **no new locking** — cfg80211 core now takes the wiphy mutex before
    calling into any of them, where 5.10-era cfg80211 relied on the caller
    holding RTNL instead.
  - The real risk is the **other direction**: this driver calls
    `cfg80211_scan_done()` / `cfg80211_connect_result()` / `cfg80211_roamed()`
    from **firmware-event handling, not from inside an ops callback** —
    `rwnx_msg_rx.c` (async, driven by SDIO RX) and `rwnx_main.c:1212,2082,2168`
    (also async paths, not the ops entry points). Multiple `cfg80211.h` doc
    comments for adjacent notification APIs state "the caller must hold ...
    wiphy mutex" (e.g. `cfg80211.h:9428`: "Caller must hold wiphy mutex,
    therefore must only be called from sleepable context") — the pattern that
    replaced the old "hold RTNL" requirement. **Concretely: every
    `cfg80211_*` notification call reached from `rwnx_msg_rx.c` /
    the async paths in `rwnx_main.c` needs `wiphy_lock(wiphy)` /
    `wiphy_unlock(wiphy)` wrapped around it that wasn't there before** (5.10
    only needed `rtnl_lock()`, which this driver's 3 existing
    `rtnl_lock()`/`rtnl_unlock()` sites at `rwnx_main.c:5429/5433,
    6063/6075, 6097/6102` already show it knows how to take defensively — the
    fix is adding the wiphy-mutex equivalent at the async notification sites,
    not at those 3 sites, which are netdev-registration paths and likely stay
    RTNL-only).
  - This is still the single highest-effort item in the port — not because the
    contract is unknown (it's now confirmed above), but because applying it
    correctly means auditing every async→cfg80211 call site in `rwnx_msg_rx.c`
    and the async branches of `rwnx_main.c` (1212, 2082, 2168, 2445-2645) one
    by one, and because a wrong lock order (wiphy mutex vs. RTNL vs. this
    driver's own internal locks/semaphores) produces lockdep splats or
    deadlocks that only show up under real traffic, not at compile time. Cross-
    check each hunk against how `radxa-pkg/aic8800` (§0) handled the same
    transition on its 6.12/6.13 port — it hit this exact wall first.

### 3.4 SDIO/MMC
No breaking changes found in `sdio_driver`, `sdio_claim_host`/`release_host`,
`sdio_readb`/`writesb`, `sdio_set_block_size` between 5.10 and 6.18 (low
research depth on this axis — treat as low-risk, smoke-test rather than
line-audit). The mainline `dw_mmc`/`dw_mmc-rockchip` host driver is already
proven on 6.18 for eMMC (`../DRIVER-PARITY.md`: `dw_mmc (eMMC) | mainline | ✅ M3`)
and `CONFIG_MMC_DW_ROCKCHIP=y` is already in the live `.config` — the SDIO
*controller* side of this port is de-risked; only the AIC8800 *card driver*
above the `sdmmc` bus is new work.

### 3.5 proc_ops
`file_operations`→`proc_ops` for procfs landed in **5.6** — already true at the
5.10 baseline this driver was written against, and no further proc_ops changes
were found 5.10→6.18. The three files using `proc_ops`/`proc_create`
(`aicwf_sdio.c`, `aic8800_btlpm.c`, `lpm.c`) should need no change here.

### 3.6 DMA
No breaking coherent/streaming DMA API changes found 5.10→6.18 relevant to this
driver; SDIO drivers ride MMC-core DMA rather than calling
`dma_alloc_coherent`/`dma_map_single` directly for the card-side data path.
Low risk, not independently line-audited — flag if the build surfaces anything.

### 3.7 Vendor/Rockchip-only helpers — real mainline gap, not a version delta
`rockchip_wifi_power()` / `rockchip_wifi_set_carddetect()` (declared in
`include/linux/rfkill-wlan.h`, implemented in `net/rfkill/rfkill-wlan.c` in the
**vendor 5.10 tree only** — this is a Rockchip-BSP-vendor subsystem, not
mainline Linux, and does not exist in `flare-edge/research/linux-6.18.46/`).
Call sites: `aic8800_bsp/aicsdio.c:515,517,556,580` and
`aic8800_fdrv/aicwf_sdio.c:1260,1262,1329,1331` (all under
`#ifdef CONFIG_PLATFORM_ROCKCHIP`, which is true for this board). **This is not
a rename — there is nothing to rename to.** Recommended fix: **stub these
calls out entirely** rather than port `rfkill-wlan.c`. The DT already declares
a standard mainline `mmc-pwrseq-simple` (`sdio_pwrseq`, §1) bound via
`mmc-pwrseq = <&sdio_pwrseq>`, which the mainline MMC core already drives at
bus-scan/power-up time through `drivers/mmc/core/pwrseq_simple.c` — a real
mainline mechanism doing the same job (GPIO power/reset sequencing) these
vendor calls were a pre-DT-pwrseq-era stand-in for. `rockchip_wifi_set_carddetect()`
is likewise redundant for a `non-removable` MMC device, which mainline already
auto-rescans. Treat every call site as `#if 0`/deleted, not ported.
`get_cpu_version`/`rockchip_soc_id`-style helpers: **not called anywhere** in
this driver (checked, no hits) — the task's assumption that this driver calls
such a helper does not hold; no action needed. `module_param`, `kthread_run`/
`kthread_should_stop`: stable, unaffected — used extensively (RX/TX kthreads in
`aicwf_sdio.c`), no changes needed.

### 3.8 Firmware loading
`request_firmware`/`request_firmware_nowait`/`release_firmware`: stable
5.10→6.18, no signature changes found. Note this driver's actual firmware load
path is `AIC_FW_PATH`-relative direct file read via its own loader
(`aic_load_fw`/`CONFIG_USE_FW_REQUEST` is `?= n` in the Makefile — the vendor
driver does **not** use the standard `request_firmware()` API by default, it
reads firmware files from a configurable path itself). Confirm this stays true
after the port; it's a deliberate vendor choice, not a version-driven gap.

## 4. File / config / DT port plan

### 4.1 Files to bring into the 6.18 tree
Copy `aic8800dc/{aic8800_bsp,aic8800_fdrv,aic8800_btlpm}/*.{c,h}` (source only —
exclude every generated `.o`/`.ko`/`.mod.*`/`.cmd`/`Module.symvers`/
`modules.order` artifact already sitting in the vendor tree from prior
out-of-tree builds) into the 6.18 tree at:
```
flare-edge/research/linux-6.18.46/drivers/net/wireless/aic8800/
  Kconfig                    # new: top-level "source" wrapper, see 4.2
  Makefile                   # obj-y for the 3 subdirs, preserving bsp→fdrv→btlpm link order
  aic8800_bsp/    (from aic8800dc/aic8800_bsp/)
  aic8800_fdrv/   (from aic8800dc/aic8800_fdrv/, minus usb_host.c/rwnx_pci.*/rwnx_mesh.* — dead
                   code on this board's SDIO-only, non-mesh config; keep out unless a build
                   error proves them referenced elsewhere)
  aic8800_btlpm/  (from aic8800dc/aic8800_btlpm/)
```
This is exactly the path the vendor's own `Kconfig` already assumes
(`source "drivers/net/wireless/aic8800/aic8800_fdrv/Kconfig"`, §1) — no path
rewriting needed inside the sub-Kconfigs. Firmware blobs (§5) go to
`aic8800dc_fw/` under the rootfs firmware path, not into the kernel tree.

Apply, in this order, on top of the copied source: the `0001` queue_sz clamp
patch, and the Tier-1 SDIO-wakeup-sleep patch (currently only tracked under
`flare-edge/future-features-2/sdk-patches/wifi/patches/0002-aic8800dc-sdio-wakeup-sleep-not-spin.patch`
— pull it from there; it is *not* in this branch's own `sdk-patches/wifi/`, a
separate housekeeping gap worth closing regardless of this port). Then apply
the API-delta fixes from §3.

### 4.2 Kconfig wiring
Add one line to `flare-edge/research/linux-6.18.46/drivers/net/wireless/Kconfig`
(alongside the existing `source "drivers/net/wireless/broadcom/Kconfig"` etc.
list, `Kconfig:22-38`):
```
source "drivers/net/wireless/aic8800/Kconfig"
```
New `drivers/net/wireless/aic8800/Kconfig` — carry the vendor's `aic8800dc/Kconfig`
content forward nearly verbatim (it already has the right shape: an
`AIC_WLAN_SUPPORT` bool gate, an `AIC_FW_PATH` string default, and `source`
lines for the fdrv/btlpm sub-Kconfigs) but change the two `tristate` symbols in
`aic8800_fdrv/Kconfig` (`AIC8800_WLAN_SUPPORT`) and `aic8800_btlpm/Kconfig`
(`AIC8800_BTLPM_SUPPORT`) to **default `y`**, and set `AIC_WLAN_SUPPORT`
default `y` — per the task's requirement, everything built in, not modular,
since the 6.18 rootfs has no working module-loading pipeline yet
(`../PORT-STATUS.md` M3 note: the old 5.10 `.ko`s already fail on 6.18 from
vermagic mismatch, and a rebuilt 6.18 module tree for Buildroot doesn't exist
yet either).

### 4.3 Config symbols (`=y`, built-in)
```
CONFIG_WIRELESS=y          # already =y in the live 6.18 .config
CONFIG_CFG80211=y          # already =y — flipped in "batch2" per ../DRIVER-PARITY.md
CONFIG_WLAN=y               # already =y
CONFIG_MMC=y                 # already =y
CONFIG_MMC_DW_ROCKCHIP=y     # already =y
CONFIG_RFKILL=y              # already =y
CONFIG_AIC_WLAN_SUPPORT=y        # new
CONFIG_AIC8800_WLAN_SUPPORT=y    # new
CONFIG_AIC8800_BTLPM_SUPPORT=y   # new
CONFIG_BT=y                  # currently =m in the live .config — flip to =y for the same
                              #   no-working-module-pipeline reason as the aic8800 pieces
CONFIG_BT_HCIUART=y          # currently =m — flip alongside CONFIG_BT
CONFIG_BT_HCIUART_H4=y       # already =y (H4 is the transport this board's BT actually uses,
                              #   per hardware-86-panel.md: UART1/ttyS1, hciattach -s 1500000
                              #   ... any 1500000 flow nosleep)
CONFIG_CRYPTO_ARC4=y CONFIG_CRYPTO_CTR=y CONFIG_CRYPTO_CCM=y CONFIG_CRYPTO_AES=y  # driver's
                              #   internal key-handling deps, currently "⬜ batch2" in
                              #   ../DRIVER-PARITY.md — confirm =y, not =m, alongside this work
```
Do **not** enable `CONFIG_MAC80211` for this driver — confirmed by source
inspection (§3.3 note, no `ieee80211_hw`/mac80211 symbol usage anywhere in
`aic8800_fdrv/`) that this is a pure `cfg80211_ops` full-MAC driver; mac80211
was only ever needed for the *other* vendor WiFi chips in the shared
`sdk/sysdrv/drv_ko/wifi/` tree (RTL8189FS etc.), not this one. Leaving it out
avoids pulling in a subsystem this port doesn't need. Do **not** carry
`CONFIG_RFKILL_RK` — confirmed unused (`aic8800_btlpm/rfkill.c` calls the
standard mainline `rfkill_register()`, no Rockchip-specific rfkill hook).

### 4.4 DT changes
On the board DT that M4/M5 work extends
(`warden-sdk/kernel/rv1106-enablement/dts/rv1106-warden.dts` or its successor —
follow the pattern of the M3 `&emmc` addition in `rv1106-warden-m2.dts:126-142`):
1. Port the `sdmmc0_{clk,cmd,bus4,det}` pinctrl group from
   `sdk/sysdrv/source/kernel/arch/arm/boot/dts/rv1106-pinctrl.dtsi:700-731`
   (not yet in the ported tree — M1–M3 only needed the eMMC/uart2/eth pin
   groups).
2. Add the `sdmmc: mmc@ffaa0000` node (clocks `HCLK_SDMMC`/`CCLK_SRC_SDMMC` off
   `&cru`, `SCLK_SDMMC_DRV`/`SCLK_SDMMC_SAMPLE` off `&grf_cru` — the same
   `grf_cru` dependency the M2 eMMC fix already established, so no new
   prerequisite) and the `sdio_pwrseq` node, both transplanted verbatim from
   §1's block (interrupt `GIC_SPI 52`, per `rv1106.dtsi:1403-1412` in the
   vendor tree).
3. `status = "okay"` on `&sdmmc`, matching the vendor board DT exactly (§1).
4. **Do not** touch `&sdio` (mmc@ff9a0000) — leave `disabled`, it's genuinely
   unused hardware on this board (§1 correction).

### 4.5 Driver init-order note
The 5.10 module load order (`insmod_wifi.sh`, §1) is
`aic8800_bsp → (200ms) → aic8800_fdrv → (2s!) → aic8800_btlpm`, with real sleep
delays between each. Built-in (`=y`), there is no equivalent explicit delay —
initcall order is controlled by link order (the vendor Makefile's `obj-y` list
is already `aic8800_btlpm/ aic8800_fdrv/ aic8800_bsp/` in Makefile-declaration
order but Kbuild links `obj-y` in Makefile order regardless of which appears
first in the `ifeq` block — **preserve `aic8800_bsp` before `aic8800_fdrv`
before `aic8800_btlpm` in the new `drivers/net/wireless/aic8800/Makefile`'s
`obj-y` list**, mirroring the working insmod order) and by each subsystem's
declared `module_init()`/initcall level (all three currently use plain
`module_init()`, which becomes `device_initcall` level when built-in — same
level for all three, so link order is what decides relative sequencing among
them). The observed 2-second gap between `aic8800_bsp` and `aic8800_fdrv` on
the running 5.10 system is suspicious enough (firmware download + chip
bring-up time) that if the built-in probe races ahead of firmware readiness,
watch for it specifically during bring-up (§6) — this is a real risk the
static-link change introduces that modular loading didn't have, not just a
formality.

## 5. Firmware notes

Firmware is not open source (binary blobs, as expected for WiFi/BT RF/PHY
patches) but is freely redistributable — no export-control/NDA marking found on
the files themselves or in the vendor tree's licensing. 21 files,
`sdk/sysdrv/drv_ko/wifi/aic8800dc/aic8800dc_fw/` (484 KB total):
```
aic_userconfig_8800dc.txt, aic_userconfig_8800dw.txt      # text config, not firmware
fmacfw_calib_8800dc_{u02,h_u02,hbt_u02}.bin                # full-MAC calibration firmware
fmacfw_patch_8800dc_{u02,h_u02,hbt_u02,ipc_u02}.bin        # full-MAC patch firmware
fmacfw_patch_tbl_8800dc_{u02,h_u02,hbt_u02,ipc_u02}.bin    # patch tables
fw_adid_8800dc_{u02,u02h}.bin                              # analog/RF ADID cal data
fw_patch_{8800dc_u02,8800dc_u02_ext0,8800dc_u02h}.bin      # BT patch firmware
fw_patch_table_8800dc_{u02,u02h}.bin                       # BT patch tables
lmacfw_rf_8800dc.bin                                       # RF test-mode firmware
```
Per `aic_bsp_main.c`'s `fw_8800dc_u02[]` table (§1), the exact subset loaded at
runtime depends on `AICBSP_CPMODE_WORK` vs `_TEST` and on `CONFIG_SDIO_BT`
(=n in the vendor Makefile — WiFi and BT firmware are loaded/attached
separately, not as one combo blob, despite the combo chip). **Firmware load
path**: `AIC_FW_PATH` Kconfig default is `/oem/usr/ko/aic8800dc_fw` — this is a
5.10-era Buildroot-rootfs-layout artifact (the `/oem` partition), not a kernel
concept; on the 6.18 rootfs, point this at wherever WardenOS's 6.18 Buildroot
userspace places firmware (likely `/lib/firmware/aic8800dc/` if following
standard mainline convention, or keep `/oem/usr/ko/aic8800dc_fw` if the 6.18
rootfs partition layout is unchanged from 5.10 — confirm against whatever the
M4/M5 rootfs build actually produces, this is a rootfs-layout decision, not a
kernel one). Not found in `linux-firmware.git` (the mainline firmware
project) — AIC8800 firmware has not been upstreamed there; continue shipping
it the way the vendor tree already does (bundled alongside the driver, loaded
by direct file read per §3.8, not `request_firmware()`).

**Chip variant**: **AIC8800DC**, confirmed authoritative by the board config
(`RK_ENABLE_WIFI_CHIP=AIC8800DC`) and the SDIO device ID match in source
(`aicsdio.c:75,80`: vendor `0xc8a1`, device `0xc08d`) — not the dual-band
AIC8800D80 seen on other LuckFox boards. 2.4 GHz only in practice (confirmed
on hardware per `wifi-bluetooth-aic8800.md:10`: `iw phy` shows 1 band, 0
5 GHz channels, despite AIC8800DC being marketed dual-band-capable elsewhere).
"AIC8800DW" appears loosely in some sources for the same part; the board `.mk`
is authoritative.

## 6. Verify steps

Follow the existing A/B `_b`-slot hardware-verification loop
(`warden-sdk/kernel/docs/m2-boot-on-c8a3.md`), same method M1–M3 already used:
1. **Build**: driver compiles clean into the 6.18.46 tree (`make ... modules`
   is not the target — it's `=y`, so this is just `make zImage`/whatever M2's
   `build-m2.sh` wraps, succeeding with the new `drivers/net/wireless/aic8800/`
   objects linked into `vmlinux`/the zImage).
2. **Probe**: boot on `warden-c8a3`, confirm in `dmesg`: the `sdmmc` MMC host
   binds (`dwmmc_rockchip ffaa0000.mmc: ...`, same pattern M3 already proved
   for `ffa90000.mmc`/eMMC), then an SDIO card enumerates on it, then
   `aic8800_bsp`'s probe fires (vendor/device ID match, firmware file opens
   succeed — watch specifically for `AIC_FW_PATH` resolution failures, §5),
   then `aic8800_fdrv` attaches and **`wlan0` appears** in `ip link`.
3. **cfg80211 sanity**: `iw phy` shows the expected single 2.4 GHz band/14
   channels (matching the known-good 5.10 baseline in
   `wifi-bluetooth-aic8800.md:10` — a mismatch here is a signal something in
   the cfg80211/wiphy port is wrong, not a chip regression).
4. **Scan**: `iw dev wlan0 scan` returns nearby APs — exercises
   `rwnx_cfg80211_scan`/`cfg80211_scan_done()` and, indirectly, whether the
   wiphy-locking port (§3.3) is functioning rather than deadlocking.
5. **Connect**: associate to a real AP (`wpa_supplicant`/`wpa_cli`, matching
   WardenOS's own `wlan0` ownership model — do **not** start `rkwifi_server`,
   §1) and confirm `COMPLETED` state plus a DHCP lease — exercises
   `rwnx_cfg80211_connect`/`cfg80211_connect_result()`.
6. **Known-regression checks** (carry forward, don't re-discover): confirm the
   two hardening patches (§0/§4.1) are actually effective — no console-flood
   "cmd timed-out" spam under load, and no CPU-pinning busy-spin if the SDIO
   link is stressed (`aicwf_bustx_thr` should sleep, not spin, on a wakeup
   failure). Confirm `iw dev wlan0 set power_save off` still behaves as
   expected (a wall-powered panel, no reason to want power-save —
   `wifi-bluetooth-aic8800.md:41`).
7. **BT** (secondary to WiFi but same milestone): confirm `aic8800_btlpm`
   attaches and `hciattach -s 1500000 /dev/ttyS1 any 1500000 flow nosleep`
   (the known-good invocation) brings up an HCI device.
8. **Update on landing**: mark the `AIC8800 wifi (bsp/fdrv)` and
   `AIC8800 BT (btlpm)` rows in `../DRIVER-PARITY.md` ✅, with the same
   "hardware-verified, not just compiled" bar every other row uses.

## Sources

- Local (SDK/repo evidence, cited inline above by path:line):
  `flare-edge/sdk/sysdrv/drv_ko/wifi/aic8800dc/**`,
  `flare-edge/sdk/sysdrv/drv_ko/wifi/Makefile`,
  `flare-edge/sdk/sysdrv/source/kernel/arch/arm/boot/dts/{rv1106.dtsi,rv1106-pinctrl.dtsi,rv1106g-luckfox-pico-86panel.dts}`,
  `flare-edge/sdk/sysdrv/source/kernel/arch/arm/configs/{rv1106-sdiowifi.config,luckfox_rv1106_linux_defconfig}`,
  `flare-edge/*/sdk-patches/wifi/patches/*.patch`,
  `flare-edge/research/linux-6.18.46/.config` (live, post-M3, batch2 CFG80211=y),
  `flare-edge/research/linux-6.18.46/include/net/cfg80211.h` (directly read to
  verify `wiphy_new`, `cfg80211_connect_result`, `wiphy_lock`/`lockdep_assert_wiphy`
  against the actual target tree, not release notes — §3.3),
  `flare-edge/research/plan44-openwrt/` (checked, no aic8800),
  `warden-sdk/kernel/rv1106-enablement/{PORT-STATUS.md,DRIVER-PARITY.md,OVERNIGHT-PLAN.md}`,
  `warden-sdk/kernel/docs/{bringup.md,m2-boot-on-c8a3.md}`,
  `luckfox-pico-86-panel/{wifi-bluetooth-aic8800.md,hardware-86-panel.md,alternative-bsps.md,sdk-patches.md,boot-chain.md}`.
- Web (via research pass, see §0/§2): github.com/radxa-pkg/aic8800 +
  deepwiki.com/radxa-pkg/aic8800; forum.armbian.com topic 50332 ("AIC8800 wifi
  sdio module not working with kernel 6.12+"); kernel.org/patchwork commits
  `9b13df3fb64e` (timer_delete rename), `2655926aea9b` (netif_rx_ni removal),
  `a05829a7222e` (wiphy-lock RTNL migration start); kernelnewbies.org/Linux_6.12;
  LKML/lkml.iu.edu mirrors for the timer-rename and wiphy-guard series.
  Patchwork/LWN direct fetches were partially blocked by anti-bot walls during
  research, so the *exact commit/version* the wiphy-lock migration landed in
  is not pinned precisely — but its **presence and current contract in the
  actual 6.18.46 tree we're porting to is directly confirmed** (previous
  paragraph), which is the fact that actually matters for this port.
