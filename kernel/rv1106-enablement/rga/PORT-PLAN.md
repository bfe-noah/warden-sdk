# M6 — RGA 2D accelerator port to 6.18

Target: `/dev/rga` present and working on the self-built Linux 6.18.46 RV1106 port
(`../PORT-STATUS.md`, `../DRIVER-PARITY.md` row "RGA 2D (rga2)"), at parity with the
running vendor 5.10 kernel and with zero WardenOS UI-side code changes.

## Decision: port the vendor char-dev driver. Do not touch mainline V4L2 rga.

**WardenOS depends on the vendor `/dev/rga` ioctl uapi via librga's IM2D API — not
V4L2.** `ui-src/src/warden/warden_rga.c` (`flare-edge/major-app-additions/`) includes
`<rga/rga.h>` and `<rga/im2d.h>` and calls `wrapbuffer_fd_t()`, `improcess(..., IM_SYNC)`
and `querystring(RGA_VERSION)` (warden_rga.c:46-48, 377-390, 458, 614-631) — the
Rockchip **im2d/librga** user-space API, which talks to the kernel purely through the
vendor char-device ioctl protocol (`RGA_IOC_REQUEST_SUBMIT` etc., see below). There is
no V4L2 (`/dev/videoN`, `VIDIOC_*`) code anywhere in WardenOS's RGA path, and no
V4L2-backed librga build exists upstream to switch to even if we wanted one. Porting the
vendor driver is therefore not a preference, it is the only path that keeps the existing
UI code (and its measured 20%→8% CPU win, warden_rga.c:419) working unmodified.

The candidate alternative — adding an `rv1106` compatible string to mainline's
`drivers/media/platform/rockchip/rga/` V4L2 M2M driver — is rejected on both counts the
task asked to check:

1. **Register incompatibility.** Mainline's driver (current `torvalds/linux` master,
   representative of 6.18) matches only `rockchip,rk3288-rga`, `rockchip,rk3399-rga`
   (both mapped to one `rga2_hw` struct, `.features = FLIP | ROTATE | BG_COLOR` only —
   no scaling, no format conversion) and `rockchip,rk3588-rga3`. Per Rockchip's own FAQ
   ("Although RGA on both RK3399 and RV1126 is RGA2-ENHANCE, their sub versions are
   different" — `librga/docs/Rockchip_FAQ_RGA_EN.md`, Q2.10) RK3399's RGA is already
   **RGA2-ENHANCE**, a newer/richer core than RK3288's baseline RGA2 that mainline's
   `rga2_hw` was written against — so mainline's claim of rk3399 compatibility is itself
   the reduced-feature subset, not proof of a shared register map. RV1106 is a *third*
   point in that family: our own vendor driver (`rga_drv.c:1402-1409`) special-cases the
   exact hardware version string `"3.3.87975"` to a distinct `rga2e_1106_data` table,
   separate from the generic `rga2e_data` and from the IOMMU-capable `rga2e_iommu_data`
   used by other RGA2-ENHANCE chips — i.e. even Rockchip's own driver does not treat
   RV1106 as register-identical to its closest relatives, let alone to RK3288/RK3399's
   older baseline core. A pending upstream series (Jianfeng Liu,
   `20240322052915.3507937-1-liujianfeng1994@gmail.com`, "media: rockchip: rga: Add
   rk3568 support") explicitly states "RGA2 on rk3568 is the same core as RGA2 on
   rk3288" — confirming mainline's whole `rga2_hw` lineage targets the *old* core, not
   the ENHANCE family RV1106 belongs to.
2. **Feature/uapi mismatch.** Even where mainline's driver runs, it only implements
   flip/rotate/solid-fill via V4L2 M2M — no im2d, no `/dev/rga` char device, no
   `wrapbuffer_fd_t`/`improcess` surface. Adopting it would mean rewriting
   `warden_rga.c`'s draw-unit and buffer-sync-copy paths against `VIDIOC_*` ioctls from
   scratch, on hardware nobody has shown is even electrically the same core.

## SDK source map (vendor 5.10, what we are porting)

Real path: `flare-edge/sdk/sysdrv/source/kernel/drivers/video/rockchip/rga3/`
(`sdk` is a symlink to `<flare-edge>/sdk`). This is Rockchip's
**"multicore" RGA driver** (`CONFIG_ROCKCHIP_MULTI_RGA`, module name `rga3.ko`/built-in
`rga3.o`) — open source, `SPDX-License-Identifier: GPL-2.0`, driver version **1.3.1**
(`include/rga_drv.h:88-94`; the current `airockchip/librga` upstream ships a matching
**1.3.3** for RK3588 on 6.1/6.4 Armbian kernels — same lineage, already proven building
against 6.x elsewhere). There are three other RGA driver trees in the SDK
(`drivers/video/rockchip/rga/`, `rga2/`, and the mainline-style
`drivers/media/platform/rockchip/rga/`) — **none of these are built** for RV1106
(`grep CONFIG_ROCKCHIP_MULTI_RGA arch/arm/configs/*rv1106*defconfig` is the only RGA
symbol set; `drivers/media/platform/rockchip/rga` isn't referenced by any rv1106
defconfig or DT). Ignore them; `rga3/` is the one true source.

| File | Role |
|---|---|
| `rga_drv.c` | probe/remove, misc char-dev (`"rga"` → `/dev/rga`), ioctl dispatch, IRQ, clocks, hrtimer scheduler tick |
| `rga_common.c`, `rga_job.c`, `rga_mm.c` | job/request lifecycle, buffer-descriptor management |
| `rga2_reg_info.c`, `rga3_reg_info.c`, `rga_hw_config.c` | per-core register programming (the actual "how to talk to the silicon" — this is what a mainline single-core driver does NOT have for RGA2-ENHANCE) |
| `rga_dma_buf.c` | dma-buf import/map (`dma_buf_attach` + `dma_buf_map_attachment`, `iosys_map`) |
| `rga_iommu.c` | IOMMU attach — **not exercised on RV1106** (see below) |
| `rga_fence.c` | `dma_fence`/`sync_file` for the async ioctl path — **not needed**, WardenOS is sync-only (see Scope cuts) |
| `rga_debugger.c` | optional procfs/debugfs introspection |
| `include/rga.h` | uapi: ioctl numbers or `RGA_IOC_MAGIC='r'` — `RGA_IOC_GET_DRVIER_VERSION`, `RGA_IOC_GET_HW_VERSION`, `RGA_IOC_IMPORT_BUFFER`, `RGA_IOC_REQUEST_CREATE/SUBMIT/CONFIG/CANCEL`, plus the legacy `RGA_BLIT_SYNC`(0x5017)/`RGA_BLIT_ASYNC`/`RGA_GET_VERSION` numeric ioctls. **This is the exact uapi `librga.so` (userspace) calls** — nothing here changes; we port the kernel side only. |

**DT match for our chip:** `rga2_dt_ids[]` (`rga_drv.c:1249-1258`) matches
`compatible = "rockchip,rga2_core0"` → `rk3588_rga2_match_data` (clock names
`"aclk_rga2","hclk_rga2","clk_rga2"`, `rga_drv.c:1195-1199`) — this is **exactly** our
vendor DT node:
```c
// rv1106.dtsi:1154-1161
rga2: rga@ff980000 {
    compatible = "rockchip,rga2_core0";
    reg = <0xff980000 0x1000>;
    interrupts = <GIC_SPI 87 IRQ_TYPE_LEVEL_HIGH>;
    clocks = <&cru ACLK_RGA2E>, <&cru HCLK_RGA2E>, <&cru CLK_CORE_RGA2E>;
    clock-names = "aclk_rga2", "hclk_rga2", "clk_rga2";
    status = "disabled";
};
```
No `resets =`, no `iommus =` on this node — confirmed correct: `rga_drv.c` never calls
`reset_control_*` at all (unlike VOP, which needed named resets added for mainline,
`../display/README.md`), and probe only calls `rga_iommu_probe()` when
`scheduler->data->mmu == RGA_IOMMU` (`rga_drv.c:1420-1425`) — for RV1106's matched
`rga2e_1106_data` (selected by exact HW-version-string match `"3.3.87975"`,
`rga_drv.c:1402-1409`) that table has no IOMMU, so the branch never runs. **RV1106's RGA2
is physically-contiguous-only**, which is exactly why `warden_rga.c` allocates its
canvas/scanout buffers from `/dev/dma_heap/cma` (CMA dma-buf heap) rather than any
generic malloc — no driver-side change needed here, the existing WardenOS allocation
strategy already matches the hardware constraint.

**IRQ name:** `dev_driver_string(dev)` = the platform_driver's `.driver.name`, which for
the `rga2_dt_ids` match is literally `"rga2"` (`rga_drv.c:1478-1481`) — this is the exact
string that shows up as `rga2` in `/proc/interrupts` on the running 5.10 image, confirming
the task's framing and that we're looking at the right driver.

**DT enable status (5.10, for parity):** the shipped 86-Panel board DT already turns this
node on — `rv1106g-luckfox-pico-86panel.dts` → `rv1106-luckfox-pico-86panel-ipc.dtsi` →
`#include "rv1106-evb.dtsi"` → `&rga2 { status = "okay"; };` (`rv1106-evb.dtsi:59-61`).
`sdk-patches/kernel/configs/flare-edge.config:21-29` (flare-edge repo) is the config
fragment: `CONFIG_ROCKCHIP_MULTI_RGA=y` (built-in, not `=m`, so devtmpfs creates
`/dev/rga` with no insmod step) + `CONFIG_DMABUF_HEAPS=y` + `CONFIG_DMABUF_HEAPS_CMA=y`.

## Userspace: already built, nothing to port

`librga.so`/`.a` for our exact target triple already exists prebuilt in the SDK:
`sdk/media/rga/release_rga_rv1106_arm-rockchip830-linux-uclibcgnueabihf/lib/librga.so`
(Apache-2.0 licensed headers, `rga.h`/`im2d.h`). It links against the kernel uapi in
`include/rga.h` above, which is unchanged by this port. Once `/dev/rga` exists with the
same ioctl numbers, the existing `librga.so` and the existing `warden_rga.c` binary/object
need **no changes** — this is a pure kernel-side port.

## API-delta checklist, 5.10 → 6.18 (checked against the real target tree)

Checked directly against `flare-edge/research/linux-6.18.46/` (the tree M1-M3 already
build and boot on), not guessed:

| # | Delta | Evidence | Fix |
|---|---|---|---|
| 1 | **`platform_driver.remove` is now `void`, not `int`.** | `linux-6.18.46/include/linux/platform_device.h:233`: `void (*remove)(struct platform_device *);`. Vendor `rga_drv_remove()` (`rga_drv.c`) returns `int`. Used by all 3 `platform_driver` structs (rga3_core0/1, rga2). | Change signature to `void`, drop the `return ret;`/`return 0;`, keep the body. Mechanical, 1 function + 3 struct references. |
| 2 | **`hrtimer_init()` is gone; `hrtimer_setup()` merges init+callback.** | `grep hrtimer_init` on `linux-6.18.46/include/linux/hrtimer.h` returns nothing callable — only `hrtimer_setup()`/`hrtimer_setup_on_stack()` (`hrtimer.h:230-235`). Vendor `rga_drv.c:362-366` does the old split form: `hrtimer_init(&timer, CLOCK_MONOTONIC, HRTIMER_MODE_REL); timer.function = hrtimer_handler;`. | `hrtimer_setup(&timer, hrtimer_handler, CLOCK_MONOTONIC, HRTIMER_MODE_REL);` — one call site, but it's the RGA scheduler tick (`rga_drv.c:327-371`), so build-clean is necessary but not sufficient: exercise it under load (the blit test below) since a hrtimer regression shows up as stalled/duplicate completions, not a compile error. **This is the delta most likely to bite in a way the compiler won't catch — treat it as the highest-scrutiny item.** |
| 3 | `dma_buf_attach()` / `dma_buf_map_attachment()` | `linux-6.18.46/include/linux/dma-buf.h:571,588` — signatures (`dmabuf, dev`) / (`attach, dir`) → `sg_table` are **unchanged** from what `rga_dma_buf.c:443,450,494,501` already calls. Vendor code already uses `struct iosys_map` (post-5.18 API) at `rga_dma_buf.c:398`, so it's already ahead of 5.10 baseline here. | No change needed; compile-verify only. |
| 4 | `class_create()` losing its `owner` arg (6.4) | Not applicable — `rga3/` has **zero** `class_create` calls (grep across the whole dir + `include/`); it registers `/dev/rga` via `misc_register()` (`rga_drv.c:1520`), which needs no class. | Nothing to fix. (Flagged in the task brief as a risk; verified moot for this driver.) |
| 5 | IOMMU API drift (`iommu_domain_alloc`, `iommu_attach_device`, rockchip IOMMU v2) | `rga_iommu.c` uses `iommu_group_get`/`iommu_get_domain_for_dev` — but as established above, **RV1106 never calls `rga_iommu_probe()`** (its match_data has no `RGA_IOMMU` flag). | Out of scope for RV1106; don't even need to build-fix `rga_iommu.c`'s IOMMU-attach path correctness, only that it compiles (dead code on our chip). |
| 6 | `proc_ops`/`debugfs_create_file` | `rga_debugger.c:499,553,597,605,633,686` already uses `struct proc_ops` (post-5.6) and plain `debugfs_create_file`/`proc_create_data` — both stable, unchanged APIs in 6.18. | No change needed. Optional subsystem anyway (see Scope cuts). |
| 7 | GRF/syscon regmap lookups | None — `rga3/` has zero `syscon`/`regmap`/`rockchip,grf` references, unlike VOP which needed the `grf_ctx` fix in `PORT-STATUS.md` M2 item 3. | Nothing to fix; simpler than the VOP port in this respect. |

## Scope cuts (reduce port surface to exactly what WardenOS uses)

- **Do not enable `CONFIG_ROCKCHIP_RGA_ASYNC`.** `warden_rga.c`'s own header comment is
  explicit: "The RGA blit uses the synchronous path. No fences" (warden_rga.h:23), and
  every call site uses `improcess(..., IM_SYNC)` (warden_rga.c:388,629). The
  `Makefile`'s `rga3-$(CONFIG_ROCKCHIP_RGA_ASYNC) += rga_fence.o` means leaving this
  config off **excludes `rga_fence.c` (dma_fence/sync_file) from the build entirely** —
  removing an entire API-delta surface (dma_fence context allocation, sync_file
  lifetime) that WardenOS never exercises. Matches the vendor default (`ROCKCHIP_RGA_ASYNC`
  defaults to `y` in Kconfig but is safe to turn off; verify nothing else in the SDK
  userspace on this image needs it — nothing does; MPP/ISP aren't used on the 86-Panel).
- **`CONFIG_ROCKCHIP_RGA_PROC_FS`/`_DEBUG_FS`/`_DEBUGGER`: leave off initially.** Useful
  for bring-up (dumps registered buffers/jobs) but not required for `/dev/rga` to exist
  or work; add later if debugging needs it.
- **Only `rga3/` (`CONFIG_ROCKCHIP_MULTI_RGA`) is ported** — not `drivers/video/rockchip/rga/`
  or `rga2/` (the older single-core drivers), matching the vendor 5.10 build exactly.

## File / config / DT plan

### 1. Kernel source (new directory in the 6.18 tree)

`drivers/video/rockchip/` does not exist yet in `linux-6.18.46` (`ls` confirms). Create:

```
drivers/video/rockchip/Kconfig            # source "drivers/video/rockchip/rga3/Kconfig"
drivers/video/rockchip/Makefile           # obj-$(CONFIG_ROCKCHIP_MULTI_RGA) += rga3/
drivers/video/rockchip/rga3/              # forward-ported rga_drv.c, rga_common.c,
                                           #   rga3_reg_info.c, rga2_reg_info.c,
                                           #   rga_hw_config.c, rga_mm.c, rga_dma_buf.c,
                                           #   rga_iommu.c, rga_policy.c, Kconfig, Makefile
                                           #   (rga_fence.c, rga_debugger.c omitted —
                                           #   see Scope cuts)
```

Hook into the parent build (mirrors the vendor's own top-level wiring,
`sdk/.../drivers/video/{Kconfig,Makefile}`):
- `drivers/video/Kconfig`: add `source "drivers/video/rockchip/Kconfig"`
- `drivers/video/Makefile`: add `obj-y += rockchip/`

`rga3/Makefile` (trimmed for the scope cuts above):
```make
# SPDX-License-Identifier: GPL-2.0
ccflags-y += -I$(srctree)/$(src)/include
rga3-y := rga_drv.o rga_common.o rga3_reg_info.o rga_iommu.o rga_dma_buf.o \
          rga_job.o rga_hw_config.o rga2_reg_info.o rga_policy.o rga_mm.o
obj-$(CONFIG_ROCKCHIP_MULTI_RGA) += rga3.o
```
(drops the `rga3-$(CONFIG_ROCKCHIP_RGA_ASYNC) += rga_fence.o` and
`rga3-$(CONFIG_ROCKCHIP_RGA_DEBUGGER) += rga_debugger.o` lines from the vendor Makefile.)

Apply the two API-delta fixes from the table above (`platform_driver.remove` → void,
`hrtimer_init`+`.function=` → `hrtimer_setup()`) during the transplant, same
build-fix-build loop already used for `clk-rv1106.c`/`pinctrl-rockchip.c`
(`PORT-STATUS.md` M1).

### 2. Kconfig fragment (defconfig / config fragment)

```
CONFIG_ROCKCHIP_MULTI_RGA=y
CONFIG_DMABUF_HEAPS=y
CONFIG_DMABUF_HEAPS_CMA=y
```
(`CONFIG_DMABUF_HEAPS` is currently `# CONFIG_DMABUF_HEAPS is not set` in
`linux-6.18.46/.config` — confirmed by grep — so this is a required addition, not
already-on. `CONFIG_ARCH_ROCKCHIP=y` is already set.) `=y` not `=m`, matching the
vendor fragment's own rationale (`flare-edge.config:23`: builtin so devtmpfs creates
the node with no insmod step).

### 3. Devicetree — two additions to `dts/rv1106-warden.dts`

**(a) Enable the node.** `rv1106.dtsi` is already `#include`d wholesale by
`rv1106-warden.dts` (`dts/README.md`), and it already carries the `rga2` node
byte-for-byte (clocks, IRQ, compatible) — just disabled. Add the same one-line override
the vendor board DT uses:
```dts
&rga2 {
	status = "okay";
};
```
No clock/reset/iommu properties to add — all three RGA2E clocks
(`HCLK_RGA2E`=269, `ACLK_RGA2E`=270, `CLK_CORE_RGA2E`=271) are **already wired** in the
ported `clk-rv1106.c` (`clk/clk-rv1106.c:924-930`, landed in M1, `PORT-STATUS.md`) off
the same `hclk_vo_root`/`aclk_vo_root` parents that VOP already uses successfully on
hardware (M4, verified) — this is the lowest-risk clock story of any M6 driver.

**(b) Add the CMA reserved-memory pool — currently missing from the ported DT.**
`warden_rga.c`'s canvas allocator opens `/dev/dma_heap/cma`
(`warden_rga.c:70,269-274`), which requires a `linux,cma`/`shared-dma-pool`
reserved-memory node to exist. On the vendor 5.10 board DT this lives in
`rv1106-luckfox-pico-86panel-ipc.dtsi` (**not** `rv1106.dtsi`, and confirmed **not yet
present** in `dts/rv1106-warden.dts` — grep for `reserved-memory`/`linux,cma` there
returns nothing):
```dts
// rv1106-luckfox-pico-86panel-ipc.dtsi:91-107
reserved_memory: reserved-memory {
	status = "okay";
	#address-cells = <1>;
	#size-cells = <1>;
	ranges;
	linux,cma {
		status = "okay";
		compatible = "shared-dma-pool";
		inactive;
		reusable;
		size = <0xA00000>;   /* 10 MiB */
		linux,cma-default;
	};
};
```
Port just the `linux,cma` child (the sibling `drm-logo`/`mmc_ecsd` reserved regions
belong to the display/boot-logo and eMMC-ECSD milestones respectively, not RGA) into
`rv1106-warden.dts`. Without this the RGA kernel driver itself will still probe and
`/dev/rga` will still appear (the driver has no CMA dependency of its own — DMA-BUF
import works from any dma-buf exporter), but WardenOS's own canvas/scanout allocator
will fail open and the UI falls back to the CPU path silently (`warden_rga.c:269-274`
returns `-1` on `open()` failure) — so this step is required for the *offload* to be
observably working, even though it isn't required for `/dev/rga` to exist.

### 4. Rootfs / userspace

None needed — `librga.so`/`.a` for `rv1106_arm-rockchip830-linux-uclibcgnueabihf`
already ships in the SDK (`sdk/media/rga/release_rga_rv1106_.../lib/`) and is already
what the current Buildroot overlay installs and what `warden_rga.c` already links
against with `WARDEN_USE_RGA=1`. No rebuild of librga or of WardenOS's C code is
implied by this kernel port.

## Verify steps

1. **Build:** `rga3.o` compiles with zero warnings/errors against `linux-6.18.46`
   (same evidentiary bar as `PORT-STATUS.md`'s M1 entries — "COMPILES CLEAN").
2. **Boot + node:** on warden-c8a3 (A/B `_b`-slot loop, `../docs/m2-boot-on-c8a3.md`),
   confirm dmesg shows `rga2, irq = 87, match scheduler` and
   `rga2 hardware loaded successfully, hw_version:3.3.87975.` (the exact version string
   `rga_drv_probe` selects `rga2e_1106_data` on, `rga_drv.c:1403`) — a different version
   string here would mean the wrong match-data table and is a stop-ship signal.
   `ls -l /dev/rga` exists with no manual `mknod`/`insmod`.
3. **CMA heap:** `ls /dev/dma_heap/cma` exists (needs step 3(b) above).
4. **Blit test (hardware-observable, no display needed):** the simplest self-contained
   check is `librga`'s own CLI/test harness if the SDK ships one, or a ~20-line C
   program using the already-present `librga.so`: allocate two small dma-buf CMA
   buffers via `/dev/dma_heap/cma`, fill one with a known pattern, call
   `improcess(src, dst, ..., IM_SYNC)` for a straight copy, and memcmp the result —
   this exercises exactly the ioctl path (`RGA_IOC_REQUEST_SUBMIT`/`RGA_BLIT_SYNC`) and
   the hrtimer-driven completion path (API-delta #2) that WardenOS's real usage
   exercises, without needing DRM/VOP (M4) to be finished first.
5. **On-target UI evidence (once M4/display lands):** boot the real WardenOS build with
   `WARDEN_USE_RGA=1`, open the graph/Monitor page, confirm `warden_rga_available()`
   reports true (`querystring(RGA_VERSION)` succeeds — `warden_rga.c:458-466`) and the
   Monitor page's "RGA" load metric moves during graph scroll (`warden_rga_load_pct()`,
   `warden_rga.c:226-241`) — the same on-target evidence bar as every other change in
   this repo (`../../CLAUDE.md`: "UI/daemon changes are verified on a real panel").

## Summary of residual risk (PORT-VERIFY-class items)

- **hrtimer_setup() correctness under load** (delta #2) — compiles clean is not enough;
  needs the blit-test loop run repeatedly/concurrently to rule out a scheduler-tick
  regression.
- **CMA pool sizing** — 10 MiB was sized against the 5.10 image's actual usage (graph
  canvas + scanout mirror at 720×720×4B ≈ 2 MiB each); carry the same size unless a
  future accounting shows it's tight.
- **Driver-parity table** (`../DRIVER-PARITY.md`) should move `RGA 2D (rga2)` from [ ] to
  [wip]/[x] as these steps land, same convention as every other M-milestone row.

## Sources

- Vendor SDK (primary evidence for all file/line citations above):
  `flare-edge/sdk/sysdrv/source/kernel/drivers/video/rockchip/rga3/` and
  `.../arch/arm/boot/dts/{rv1106.dtsi,rv1106-evb.dtsi,rv1106-luckfox-pico-86panel-ipc.dtsi}`
- WardenOS RGA integration: `flare-edge/major-app-additions/ui-src/src/warden/warden_rga.{c,h}`,
  `flare-edge/major-app-additions/sdk-patches/kernel/configs/flare-edge.config`
- This port's own live status/conventions: `../PORT-STATUS.md`, `../DRIVER-PARITY.md`,
  `../OVERNIGHT-PLAN.md`, `../display/README.md` (VOP port, same-family precedent),
  `../clk/rv1106-cru.h`, `../clk/clk-rv1106.c`
- Target kernel tree (ground truth for the API-delta table):
  `flare-edge/research/linux-6.18.46/{include/linux/platform_device.h,hrtimer.h,dma-buf.h}`,
  `.config`
- [airockchip/librga](https://github.com/airockchip/librga) — upstream userspace IM2D
  library (Apache-2.0), RV1106 target support, 1.3.3 driver version on RK3588/6.1-6.4
- [librga FAQ — RGA hardware family notes](https://github.com/airockchip/librga/blob/main/docs/Rockchip_FAQ_RGA_EN.md) (Q2.10: RK3399/RV1126 both RGA2-ENHANCE, differing sub-versions, ROP cut on RV1126)
- [torvalds/linux — drivers/media/platform/rockchip/rga/rga.c](https://raw.githubusercontent.com/torvalds/linux/master/drivers/media/platform/rockchip/rga/rga.c) — mainline V4L2 driver's `of_device_id` table (rk3288-rga, rk3399-rga → `rga2_hw`; rk3588-rga3 → `rga3_hw`)
- [LWN — "media: platform: rga: Add RGA3 support"](https://lwn.net/Articles/1041152/) — RK3588 RGA3 mainlining, one `/dev/video` per core, no multicore scheduling in-kernel
- [lore.kernel.org — "media: rockchip: rga: Add rk3568 support" (Jianfeng Liu)](https://lore.kernel.org/lkml/20240322052915.3507937-1-liujianfeng1994@gmail.com/) — "RGA2 on rk3568 is the same core as RGA2 on rk3288" (confirms mainline's RGA2 lineage is the older/baseline core, not RGA2-ENHANCE)
- CNX Software, [Rockchip RK3588 mainline Linux support](https://www.cnx-software.com/2024/12/21/rockchip-rk3588-mainline-linux-support-current-status-and-future-work-for-2025/) — RGA2 V4L2 mainlining timeline context
