# RKNPU kernel driver → 6.18 — port plan (M6 class)

Scope: port the **kernel driver only** (`rknpu.ko`'s source, statically built into
our tree) so `/dev/dri/cardN` (or `renderD1xx`) binds on the RV1106 NPU and answers
a version-query ioctl. This does **not** run a model — see §3 for why that's a
separate, much bigger, and largely closed problem. Written against the same
target as the rest of this port: **Linux 6.18.46 vanilla**
(`flare-edge/research/linux-6.18.46/`), forward-ported from vendor 5.10.160, built
with our `arm-rockchip830-...-gcc 8.3` toolchain — see `../PORT-STATUS.md` and
`../../docs/bringup.md` for the method and milestones this slots into (M6, listed
in `../DRIVER-PARITY.md` as "NPU (rknpu, ff660000) | out-of-tree | ⬜ M6").

Builds on `warden-sdk/docs/npu-graphics-feasibility.md`, which already read this
same driver source to answer a narrower question (can the NPU do graphics — no).
This document answers the porting question that doc explicitly deferred.

---

## 1. What we're forward-porting (vendor SDK source)

Source: `flare-edge/sdk/sysdrv/source/kernel/drivers/rknpu/` — vendor version
**0.9.2** (`DRIVER_MAJOR/MINOR/PATCHLEVEL` in `include/rknpu_drv.h:34-36`,
`DRIVER_DATE "20230825"`), currently loaded on the shipping 5.10.160 kernel as
`rknpu.ko`, IRQ `ff660000.npu`.

### The driver already targets RV1106 natively — this is not a from-scratch adaptation

The vendor driver is a **single multi-SoC codebase**, not something written for
RK3588 that we'd have to adapt. `rknpu_drv.c`'s `of_match` table already carries a
dedicated RV1106 entry and config struct:

```c
// rknpu_drv.c:196-198
{ .compatible = "rockchip,rv1106-rknpu", .data = &rv1106_rknpu_config },

// rknpu_drv.c:143-160
static const struct rknpu_config rv1106_rknpu_config = {
	.dma_mask = DMA_BIT_MASK(32),
	.pc_data_amount_scale = 2,
	.pc_task_number_bits = 16,
	.pc_task_number_mask = 0xffff,
	.pc_task_status_offset = 0x3c,
	.pc_dma_ctrl = 0,
	.bw_enable = 1,
	.irqs = rknpu_irqs, .resets = rknpu_resets,
	.nbuf_phyaddr = 0, .nbuf_size = 0,        // no NBUF wired for this SoC
	.max_submit_number = (1 << 16) - 1,
};
```

And the devicetree side is **already complete** in `rv1106.dtsi` (base tree,
`status = "disabled"`) — nothing to author, only enable:

```dts
// arch/arm/boot/dts/rv1106.dtsi:1127-1138
npu: npu@ff660000 {
	compatible = "rockchip,rv1106-rknpu";
	reg = <0xff660000 0x10000>;
	interrupts = <GIC_SPI 109 IRQ_TYPE_LEVEL_HIGH>;
	clocks = <&cru ACLK_RKNN>, <&cru HCLK_RKNN>;
	clock-names = "aclk", "hclk";
	assigned-clocks = <&cru ACLK_RKNN>;
	assigned-clock-rates = <420000000>;
	resets = <&cru SRST_A_RKNN>, <&cru SRST_H_RKNN>;
	reset-names = "srst_a", "srst_h";
	status = "disabled";
};
```

No `power-domains` property (RV1106's NPU is single-core, single-rail — unlike
RK3588's 3-core NPU which needs `genpd_dev_npu0/1/2`) and no `iommus` property
(matches the 5.10 boot-log finding already on record: `"rknpu iommu device-tree
entry not found!, using non-iommu mode"`, `npu-graphics-feasibility.md:123-126`).
Both facts materially shrink the v1 port's scope — see §2.3 and §2.4.

### The uapi/job model (for context — already characterized in the feasibility doc)

`include/rknpu_ioctl.h`: `struct rknpu_task` (`regcfg_amount`, `regcfg_offset`,
`regcmd_addr`) + `struct rknpu_submit` (`task_obj_addr`, `regcfg_obj_addr`,
`core_mask`, `fence_fd`) — a **register command-list (regcmd) task-queue model**,
submitted via `DRM_IOCTL_RKNPU_SUBMIT` and executed to completion with the driver
blocking on a hardware IRQ (`rknpu_job.c`, `wait_event_timeout`). Six ioctls total:
`RKNPU_ACTION`, `RKNPU_SUBMIT`, `RKNPU_MEM_{CREATE,MAP,DESTROY,SYNC}`
(`rknpu_ioctl.h:288-322`). `RKNPU_ACTION` carries the version queries this plan's
verify step uses: `RKNPU_GET_HW_VERSION = 0`, `RKNPU_GET_DRV_VERSION = 1`
(`rknpu_ioctl.h:113-114`).

### Memory manager choice: DRM GEM (not DMA-heap)

`Kconfig` offers a mutually-exclusive choice: `ROCKCHIP_RKNPU_DRM_GEM` (default)
vs `ROCKCHIP_RKNPU_DMA_HEAP`. **Pick DRM_GEM** — M4 (VOP2 display) already pulls
the DRM core into this kernel for the panel, so there's no new subsystem cost, and
DRM_GEM is the vendor's default/most-tested path. This also means the node that
appears is a classic DRM char device (`/dev/dri/cardN` / `renderD1xx` via
`drm_dev_alloc`/`drm_dev_register`, `rknpu_drv.c:725,730`) — **not** the newer
`/dev/accel/` framework mainline's own `rocket` driver uses (§3). Don't confuse
the two node namespaces when verifying.

---

## 2. Files, config, and the concrete 5.10→6.18 API deltas

### 2.1 File list — v1 minimal port

| File | Bring in? | Why |
|---|---|---|
| `rknpu_drv.c` / `include/rknpu_drv.h` | **Yes** | probe/remove, of_match table, DRM driver registration, power get/put |
| `rknpu_job.c` / `include/rknpu_job.h` | **Yes** | job submit, IRQ handler, PC task-list execution |
| `rknpu_gem.c` / `include/rknpu_gem.h` | **Yes** | GEM memory manager (DRM_GEM path) |
| `rknpu_reset.c` / `include/rknpu_reset.h` | **Yes** | `SRST_A_RKNN`/`SRST_H_RKNN` reset control |
| `rknpu_iommu.c` / `include/rknpu_iommu.h` | Yes, but dead code path | 61 lines, self-contained, already version-gated to 6.1; harmless to carry even though `iommu_en` stays false on our non-IOMMU DT (§2.4) |
| `rknpu_debugger.c` / `include/rknpu_debugger.h` | Yes (optional) | `/proc/rknpu/load` — the Monitor page already polls this on 5.10 (`npu-graphics-feasibility.md:151-158`); keep for continuity even though wiring the UI back up is out of scope here |
| `rknpu_mem.c` | **No** | only for `ROCKCHIP_RKNPU_DMA_HEAP` — we're not using that memory manager |
| `rknpu_mm.c` / `include/rknpu_mm.h` | **No** | SRAM/NBUF allocator (`ROCKCHIP_RKNPU_SRAM`, needs `NO_GKI`); `rv1106_rknpu_config` has `nbuf_phyaddr=0, nbuf_size=0` — dead weight on this SoC |
| `rknpu_fence.c` / `include/rknpu_fence.h` | **No (v1)** | `ROCKCHIP_RKNPU_FENCE`/`SYNC_FILE` — dma-fence cross-driver sync, not needed to prove basic binding; revisit if a real workload needs fenced submission later |
| `Kconfig`, `Makefile` | **Yes, trimmed** | drop the `rknpu_mem.o`/`rknpu_mm.o`/`rknpu_fence.o` conditional lines' configs (leave the `Makefile` structure as-is — it's already `obj-$(CONFIG_...)`-gated per file, so simply not enabling those Kconfig symbols is sufficient; no Makefile edit required) |

### 2.2 Config symbols (built-in, matching this port's established pattern —
M1–M3 build everything statically to avoid the vermagic class of bug that
blocks `aic8800.ko` today, `PORT-STATUS.md:114-115`)

```
CONFIG_ROCKCHIP_RKNPU=y
CONFIG_ROCKCHIP_RKNPU_DRM_GEM=y
CONFIG_ROCKCHIP_RKNPU_DEBUG_FS=y      # depends on DEBUG_FS (already on for bring-up)
CONFIG_ROCKCHIP_RKNPU_PROC_FS=y       # /proc/rknpu/load continuity
# leave off for v1: ROCKCHIP_RKNPU_DMA_HEAP, ROCKCHIP_RKNPU_SRAM, ROCKCHIP_RKNPU_FENCE
CONFIG_DRM=y                          # already required by M4 (VOP2)
```

### 2.3 DT change

One-line status flip on the board DT (the same pattern M2/M3 used for
`grf-clock-controller` and the eMMC node — override in the board file, don't
touch the base `rv1106.dtsi`):

```dts
&npu {
	status = "okay";
};
```

No new properties needed — `compatible`/`reg`/`interrupts`/`clocks`/`resets` are
already correct and match the driver's own `rv1106_rknpu_config` exactly (§1).
**Do not add an `iommus =` property for v1** — see §2.4.

### 2.4 The real build blocker: four vendor-only `soc/rockchip/*.h` headers

This is the one item in this plan that isn't "already handled by the vendor's own
version gates" — verified directly against our `flare-edge/research/linux-6.18.46/`
tree, not assumed:

```c
// rknpu_drv.c:37-40 (inside #ifndef FPGA_PLATFORM, which is never defined for
// our build — grep of Makefile/Kconfig shows no FPGA_PLATFORM define anywhere)
#include <soc/rockchip/rockchip_iommu.h>
#include <soc/rockchip/rockchip_opp_select.h>
#include <soc/rockchip/rockchip_system_monitor.h>
#include <soc/rockchip/rockchip_ipa.h>
```

`find flare-edge/research/linux-6.18.46/include -iname 'rockchip_{iommu,opp_select,
system_monitor,ipa}.h'` returns **nothing** — all four are Rockchip downstream-BSP
convenience headers (DVFS/OPP-table selection, thermal/system-monitor
registration, IPA power-model, and a vendor wrapper around the IOMMU-core API)
that were never upstreamed. `rknpu_drv.h:21-23` pulls in `rockchip_opp_select.h`
unconditionally too, gated only by `KERNEL_VERSION(5,10,0) <= LINUX_VERSION_CODE`
— true for 6.18, so it's compiled by default, not something a Kconfig toggle
avoids.

**All four are genuinely dead code for RV1106 at runtime**, which is what makes
this a small, well-scoped fix rather than a real feature to build:
- `rockchip_iommu_is_enabled()` (`rknpu_drv.c:902`) is only called inside
  `if (rknpu_dev->multiple_domains)` — true only for RK3588's 3-core NPU; RV1106
  never sets it (no `power-domains` property, §1).
- The OPP/system-monitor/IPA calls drive dynamic frequency/voltage scaling and
  thermal cooling-device registration against an OPP table — RV1106's DT pins a
  single fixed clock rate (`assigned-clock-rates = <420000000>`) and has no
  `operating-points-v2` table; none of this is exercised today either.

**Fix**: add small local compat shim headers (in this port's own include path,
ahead of the vendor source's include search path) providing just the symbols
these call sites reference — `rockchip_iommu_is_enabled()` returning `false`, and
no-op/`-ENOTSUPP` stand-ins for the opp/monitor/ipa registration calls actually
referenced in `rknpu_drv.c`. This is the same "compat shim for a header that moved
or doesn't exist upstream" pattern already used for `clk-rv1106.c`'s
`panic_notifier_list` move (`../PORT-STATUS.md:18-20`) — same class of fix,
same low risk, because the code behind it is provably dead for this SoC's DT.
**Do not** reach for `#define FPGA_PLATFORM` as a shortcut — that macro also
guards the reset-control logic in `rknpu_reset.c` (nearly the whole file is
`#ifndef FPGA_PLATFORM`), which we need live; it's too blunt an instrument here.

### 2.5 Surfaces already handled by the vendor driver's own version gates
(verified against 6.18.46 headers directly, not assumed)

The driver was already written to track multiple kernel versions
(`npu-graphics-feasibility.md:264-269` first flagged this). Checked what's
actually still true at 6.18:

| Vendor gate (`rknpu_drv.c`) | 6.18 status (verified) |
|---|---|
| `#if KERNEL_VERSION(6,1,0) > LINUX_VERSION_CODE` around `.gem_free_object_unlocked` | Correctly **skipped** — that field is gone from `struct drm_driver` in 6.18's `include/drm/drm_drv.h` (grepped, zero hits), and the driver's `#else` branch already uses the modern `struct drm_gem_object_funcs` (`.free`, `.export`, `.get_sg_table`, `.vmap`, `.vunmap`, `.mmap` — `rknpu_gem.c:352-358`) |
| `DEFINE_DRM_GEM_FOPS(...)` (6.1+) vs hand-rolled `file_operations` | 6.1+ branch applies; macro is a standard DRM-core helper, present in 6.18 |
| `.gem_prime_mmap = drm_gem_prime_mmap` (6.1+) vs a custom `rknpu_gem_prime_mmap` | 6.1+ branch applies |
| `struct drm_driver` fields the vendor initializer sets (`major`, `minor`, `patchlevel`, `driver_features`, `dumb_create`, `dumb_map_offset`) | All still present in 6.18's `drm_drv.h` (line-checked) |
| `DRM_IOCTL_DEF_DRV(...)` macro (ioctl table) | Still defined in 6.18's `include/drm/drm_ioctl.h:151` |
| `iommu_map()` / `iommu_unmap()` / `iommu_get_domain_for_dev()` / `iommu_attach_device()` / `iommu_detach_device()` (`rknpu_gem.c`, `rknpu_reset.c`) | `iommu_map()`'s extern signature is unchanged in 6.18's `include/linux/iommu.h:914` (mainline did add a newer `iommu_map_nosync()` alongside it, but didn't remove the classic call) — moot anyway since this path is dead on our non-IOMMU DT (§2.4) |
| `devm_reset_control_get`, `clk_bulk_data`, `pm_runtime_get_sync`/`put_sync`/`resume_and_get` | Stable mainline APIs across the whole 5.10→6.18 span; no gate needed |

Net: outside the four-header fix in §2.4, this is expected to be a **build-fix-build
pass**, not a rewrite — confirm by actually compiling into the tree (the checks
above are header-presence/signature verification, not a build).

### 2.6 Explicitly deferred (not required to prove the driver binds)

- **IOMMU enablement.** Stays off — matches current 5.10 runtime behavior and
  avoids the newer-kernel IOMMU-core churn entirely (mainline replaced
  `iommu_domain_alloc(bus)` with device-based `iommu_paging_domain_alloc(dev)`
  somewhere in the 6.x series — confirmed by grepping 6.18.46's `iommu.h`, which
  has the new call and no bus-based `iommu_domain_alloc`). Since our DT carries no
  `iommus=` property, this churn never gets compiled against in the first place.
- **dma-fence / `ROCKCHIP_RKNPU_FENCE`.** Cross-driver sync primitive, not needed
  to answer a version-query ioctl.
- **DVFS / thermal cooling / multi-power-domain.** RV1106-inapplicable per §2.4;
  stubbed out, not implemented.
- **SRAM/NBUF allocator.** Dead weight on this SoC's config table (§2.1).

---

## 3. Verify steps (driver binding only — no model, no RKNN runtime)

Uses the same proven safe-test loop as M2/M3: the A/B `_b`-slot one-shot boot on
`warden-c8a3` (`../../docs/m2-boot-on-c8a3.md`) — never touches the working `_a`
slot, auto-reverts on hang.

1. **Build**: `CONFIG_ROCKCHIP_RKNPU=y` (+ the symbols in §2.2) added to the
   defconfig fragment; `rknpu_drv.o`/`rknpu_job.o`/`rknpu_gem.o`/`rknpu_reset.o`/
   `rknpu_iommu.o`/`rknpu_debugger.o` compile clean into `built-in.a` — this is
   where the §2.4 shim headers get proven, not just inspected.
2. **DT**: `npu@ff660000` flipped to `okay`; `dtc -W` clean, no warnings, no
   overrun of the existing DTB-placement fix from M2 (`PORT-STATUS.md`'s "place
   the fdt high" note — a bigger built-in.a makes this worth re-checking).
3. **Boot** (via the `_b`-slot loop): `dmesg | grep -i rknpu` shows the
   `platform_driver` probing without error — clock/reset/IRQ acquired, no panic,
   no `-EPROBE_DEFER` stall. Compare against the 5.10 baseline probe log for the
   same node if available.
4. **Node appears**: `ls -la /dev/dri/` shows a new `cardN`/`renderD1xx` for the
   npu — classic DRM char device (not `/dev/accel/`, see §1).
5. **Trivial ioctl, not a model**: a small host-buildable C program opens the DRM
   node and issues `DRM_IOCTL_RKNPU_ACTION` with `{.flags = RKNPU_GET_DRV_VERSION}`
   (`rknpu_ioctl.h:114`), checks the returned `value` decodes to `0.9.2`
   (`RKNPU_GET_DRV_VERSION_MAJOR/MINOR/PATCHLEVEL` macros,
   `rknpu_ioctl.h:52-54`) — and/or `RKNPU_GET_HW_VERSION` returns something
   plausible. This exercises the full ioctl-dispatch → power-get/put →
   clock/reset path with zero dependency on a regcmd buffer or the RKNN runtime.
6. **Explicitly not required for "done" here**: `DRM_IOCTL_RKNPU_SUBMIT`, any
   `.rknn` model, `librknnrt`. That's the userspace question — §4.

### Effort estimate

Smaller than M1 (clk/pinctrl — required inferring an unknown CPU-clock mux from a
sibling diff) and smaller than the display work ahead in M4 (register-map
guesswork against RV1126/RK3568 siblings). This one is closer in shape to M3 ("the
eMMC node was all M3 needed" — `PORT-STATUS.md:99`): the DT is already fully
specified upstream, the driver's C source already has a dedicated, tested RV1106
config table and of_match entry (§1), and the GEM/DRM surface is already correctly
version-gated past 6.1 (§2.5). The concentrated risk is (a) actually compiling
the §2.4 shims against real 6.18 headers rather than trusting the header-presence
check above, and (b) the possibility of additional vendor-only symbols not
surfaced by this read-through. Realistic order of magnitude: low-single-digit
engineer-days to a clean probe + version-ioctl round trip on hardware, assuming
no surprise blocks the way M2's boot-image format did.

---

## 4. The userspace-runtime reality: what porting the kernel driver does — and does NOT — unlock

The instruction that motivated this document was explicit: don't let "the driver
is open source" imply the NPU becomes open-source-usable. It doesn't. This section
updates `npu-graphics-feasibility.md §4`'s conclusion with the current
(2026-08-24) state of every open effort found.

### 4.1 The kernel driver itself: genuinely open, and this is a real forward-port of Rockchip's own code

`rknpu_drv.c` et al. are SPDX `GPL-2.0`, authored by Rockchip
(`Felix Zeng <felix.zeng@rock-chips.com>`), and are the actual vendor driver — the
same driver every Rockchip Linux SDK ships, mirrored at
[`github.com/airockchip/rknpu`](https://github.com/airockchip/rknpu) /
[`github.com/rockchip-linux/rknpu`](https://github.com/rockchip-linux/rknpu).
Porting it forward is legitimate, license-clean work, not a workaround. **This is
a different codebase from mainline's own driver** (§4.2) — don't conflate "port
the vendor driver" with "adopt mainline's `accel/rocket`"; they are unrelated
implementations of the same hardware class, and only one of them (the vendor one)
covers RV1106 at all.

### 4.2 Mainline `accel/rocket`: real, merged, and does not reach RV1106

- Merged into mainline Linux and Mesa in **2025-07** — Tomeu Vizoso, ["Rockchip NPU
  update 6: We are in mainline!"](https://blog.tomeuvizoso.net/2025/07/rockchip-npu-update-6-we-are-in-mainline.html),
  following the LKML series
  ["[PATCH v2 0/7] New DRM accel driver for Rockchip's RKNN NPU"](https://lkml.iu.edu/hypermail/linux/kernel/2502.3/02497.html).
  Present verbatim in our own vendored `flare-edge/research/linux-6.18.46/drivers/accel/rocket/`.
- **Kconfig hard-excludes RV1106 by architecture, before generation is even a
  question**: `depends on (ARCH_ROCKCHIP && ARM64) || COMPILE_TEST`
  (`drivers/accel/rocket/Kconfig` in our 6.18.46 tree). RV1106 is a 32-bit
  Cortex-A7 (`arch/arm`, confirmed on our own hardware:
  `PORT-STATUS.md`'s boot log — `CPU: ARMv7 Processor`). This is a Kconfig
  dependency, not necessarily an unfixable technical wall on its own — but it
  signals no one has done the 32-bit validation work, on top of the register-level
  work below.
- **Hardware coverage, per the driver's own docs**
  (`Documentation/accel/rocket/index.rst` in our tree): *"Hardware currently
  supported: * RK3588."* Nothing else, as shipped in 6.18.46.
- **RK3576** — active, but incomplete, and not in our tree. A 2026-07-15
  reverse-engineering effort (
  [CNX Software](https://www.cnx-software.com/2026/07/15/reverse-engineering-brings-rk3576-npu-support-to-open-source-rocket-driver-for-mainline-linux/),
  code at [`gahingwoo/linux-rk3576-npu`](https://github.com/gahingwoo/linux-rk3576-npu))
  got single-task inference working end-to-end on a Radxa ROCK 4D running Linux
  7.1-rc5 — but **multi-task chained inference (any real multi-layer network)
  fails: only the first task per NPU power session actually computes.** Not
  merged into the kernel we're building against. Cited here because it's the
  closest active precedent to "port Rocket to a new RKNPU generation," and even
  that isn't production-usable yet.
- **RK3568/RK3566** — an out-of-tree community fork exists (Armbian forum,
  ["ODROID-M1: RK3568 NPU on the open stack"](https://forum.armbian.com/topic/61651-odroid-m1-rk3568-npu-on-the-open-stack-rocket-kernel-driver-mesa-teflon/)),
  built on `accel/rocket` "with local fixes" atop the RK3588 Mesa merge request,
  requiring **byte-level comparison against captured vendor command streams** to
  work out weight-layout and CBUF differences from RK3588. Not merged upstream.
  Confirms the general pattern: **porting Rocket to a new RKNPU generation is a
  bespoke reverse-engineering project per SoC, not a recompile** — the same
  conclusion `npu-graphics-feasibility.md` already reached, now with two more
  data points (RK3576, RK3568) supporting it.
- **RV1106/RV1103** — zero hits in this research. No mainline coverage, no known
  public fork, no known RE project targeting it specifically (unlike RK3568 and
  RK3576, which both have named, in-progress efforts). This is the least-covered
  tier of the RKNPU family in the open-source world today.

### 4.3 Mesa Teflon (the TFLite delegate): entirely downstream of Rocket's coverage

Merged into Mesa 24.1 ([Phoronix](https://www.phoronix.com/news/Gallium3D-Teflon-Merged),
[docs.mesa3d.org/teflon.html](https://docs.mesa3d.org/teflon.html)) as a Gallium3D
frontend for TensorFlow Lite. Per its own docs: **"Teflon only works with etnaviv
or rocket gallium drivers."** There is no Teflon path independent of a working
Rocket kernel driver underneath it — so Teflon's real-world Rockchip coverage is
exactly Rocket's: solid on RK3588, experimental/WIP on RK3576 and (unofficially)
RK3568, absent for RV1106.

### 4.4 Independent RE efforts on the closed regcmd ISA: exploratory, not a compiler

[`phhusson/rknpu-reverse-engineering`](https://github.com/phhusson/rknpu-reverse-engineering)
("Because RKNPU only knows 4D") targets **RK3588** specifically, is in an
exploratory/documentation stage (structures like `regcfg_amount`/`regcmd_addr`
identified, DRM device enumerated) with **no compiled tool, compiler, or runtime
output**, and does not touch RV1106. This matches
`npu-graphics-feasibility.md`'s existing finding that the regcmd ISA is
undocumented outside Rockchip and reasoned-about only, not published — no new
project has changed that for any SoC generation, let alone this one.

### 4.5 Net conclusion (updated, still holds)

Porting `rknpu.ko` to 6.18 gets you an open, working **kernel-level** path: the
char/DRM device, GEM memory management, clock/reset/power sequencing, and the
raw `DRM_IOCTL_RKNPU_SUBMIT` job-queue mechanism. It does **not** get you an open
way to *produce* a valid job for that queue. For RV1106 specifically, unlike
RK3588 (has mainline Rocket + Teflon) or even RK3576/RK3568 (have active,
imperfect RE efforts), **there is no open compiler, no open runtime, and no known
public reverse-engineering project of any kind.** Every real inference workload on
this NPU has to go through the closed pipeline
(`npu-graphics-feasibility.md §1`: RKNN-Toolkit2 on a PC → `.rknn` blob →
`librknnrt`/RKNN C API on-device) for the foreseeable future — porting the kernel
driver is worth doing (it's real, bounded, evidence-backed work, same class as
RGA), but it does not change that reality, and shouldn't be scoped or sold as if
it does.

---

## Layout (this directory, once work starts)

```
npu/
  PORT-PLAN.md          this document
  compat/                (to add) local stub headers for §2.4:
    soc/rockchip/rockchip_iommu.h
    soc/rockchip/rockchip_opp_select.h
    soc/rockchip/rockchip_system_monitor.h
    soc/rockchip/rockchip_ipa.h
```
The vendor driver source itself is not duplicated here (same convention as
`clk/`, `pinctrl/`, `mach/` — vendor source stays forward-ported in
`flare-edge/research/linux-6.18.46/` as a scratch tree; only the durable delta —
compat shims, DT fragment, config fragment — belongs in this repo, captured as a
patch series once M6 actually lands).
