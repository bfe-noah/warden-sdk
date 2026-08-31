# RKNPU kernel driver: 5.10 (Rockchip vendor BSP, v0.9.2) -> 6.18.46 port

Status: **zImage + rockchip/rv1106-warden.dtb build cleanly, 0 errors, 0 warnings**,
with `CONFIG_ROCKCHIP_RKNPU=y` (+ `_DRM_GEM`/`_DEBUG_FS`/`_PROC_FS=y`) built in, 99
`rknpu`-prefixed symbols linked into `System.map`, and `&npu { status = "okay"; }`
in the board dtb. **Not flashed or probed on hardware**, per this task's explicit
boundary, this is a build-only port; on-target verification (dmesg probe log,
`/dev/dri/cardN`, the version-query ioctl round trip) is deferred to the parent
session. GPL source port only: no closed blob, same class of work as the RGA and
audio ports in this series (`../rga/PORT-DONE.md`, `../audio/PORT-PROGRESS.md`).

Trees involved:
- Target (edited): `<flare-edge>/research/linux-6.18.46/`
- Vendor source (read-only, copy-from):
  `<flare-edge>/sdk/sysdrv/source/kernel/drivers/rknpu/`
  (v0.9.2, `DRIVER_DATE "20230825"`)

Scope and strategic framing are in `PORT-PLAN.md` (file-by-file plan, authoritative)
and `OPEN-NPU-PLAN.md` (the "open driver, closed userspace" reality, this port
delivers exactly Tier A there: an open, on-hardware-verifiable *kernel* driver, and
nothing more; it does not and cannot unlock running a model).

## File set copied

Per `PORT-PLAN.md` section 2.1's "Yes" column, copied verbatim from the vendor tree into
`drivers/rknpu/` (patched in place afterward, see API-delta table below):

| File | Bring in | Notes |
|---|---|---|
| `rknpu_drv.c` / `include/rknpu_drv.h` | yes | probe/remove, of_match table (incl. the RV1106 entry + `rv1106_rknpu_config`), DRM driver registration, power get/put |
| `rknpu_job.c` / `include/rknpu_job.h` | yes | job submit, IRQ handler, PC task-list execution, `rknpu_get_hw_version()`/`rknpu_get_drv_version()` |
| `rknpu_gem.c` / `include/rknpu_gem.h` | yes | GEM memory manager (DRM_GEM path) |
| `rknpu_reset.c` / `include/rknpu_reset.h` | yes | `SRST_A_RKNN`/`SRST_H_RKNN` reset control: compiled unmodified, zero API drift |
| `rknpu_iommu.c` / `include/rknpu_iommu.h` | yes (dead code path) | 61 lines; `iommu_en` stays false on our non-IOMMU DT: compiled unmodified |
| `rknpu_debugger.c` / `include/rknpu_debugger.h` | yes | `/proc/rknpu/load` continuity target for the Monitor page: compiled unmodified |
| `rknpu_mem.c`, `rknpu_mm.c`, `rknpu_fence.c` | **no** (headers only) | DMA_HEAP / SRAM / FENCE variants: not enabled for this port (section 2.2). Their headers (`rknpu_mem.h`, `rknpu_mm.h`, `rknpu_fence.h`) *are* copied because `rknpu_drv.h`/`rknpu_gem.h` include them unconditionally for struct/prototype declarations, but zero object code results: every call site into these three is either `#ifdef CONFIG_ROCKCHIP_RKNPU_{DMA_HEAP,SRAM,FENCE}` (compiled out, all three configs left off) or `if (IS_ENABLED(CONFIG_ROCKCHIP_RKNPU_SRAM) && ...)` (compile-time-constant-folded to dead code and dropped by the optimizer, confirmed: none of `rknpu_mm_*`/`rknpu_mem_*` appear in `System.map`) |

UAPI header: `rknpu_ioctl.h` was split the way `drivers/accel/rocket/` (the mainline
Rocket driver, already vendored in this tree) does it; the real content lives at
`include/uapi/drm/rknpu_ioctl.h` (mainline convention for DRM ioctl UAPI headers),
and `drivers/rknpu/include/rknpu_ioctl.h` is a one-line redirect (`#include
<drm/rknpu_ioctl.h>`) so the vendor source's unmodified `#include "rknpu_ioctl.h"`
keeps working. This also makes the header directly usable by
`rknpu_version_test.c` (below) with a single `-I` flag and no libdrm dependency.

## Kconfig / Makefile wiring

`drivers/rknpu/Kconfig` (new), trimmed from the vendor's own `Kconfig`: drops the
`ROCKCHIP_RKNPU_SRAM`/`_FENCE` options and the `DRM_GEM` vs `DMA_HEAP` `choice`
block entirely (DRM_GEM is the only memory manager this port wires up, see
`PORT-PLAN.md` section 1 "Memory manager choice"). Sourced from `drivers/Kconfig` right
after `source "drivers/accel/Kconfig"` (RKNPU is a classic DRM device, not
`drivers/accel/`, see section 1 of `PORT-PLAN.md`, "not `/dev/accel/`").

`drivers/rknpu/Makefile` (new), trimmed from the vendor's own `Makefile`: only the
five always-on objects plus GEM (`rknpu_drv.o`, `rknpu_reset.o`, `rknpu_job.o`,
`rknpu_debugger.o`, `rknpu_iommu.o`, `rknpu_gem.o` gated on
`CONFIG_ROCKCHIP_RKNPU_DRM_GEM`); no edit needed for the never-built
`rknpu_mem.o`/`rknpu_mm.o`/`rknpu_fence.o` lines because they're simply omitted
(matches `PORT-PLAN.md` section 2.1's note that no Makefile edit is required, just not
setting their Kconfig symbols). `ccflags-y` adds `compat/` to the include path
**ahead of** `include/`. See the compat-shim section below for why the ordering
matters.

Wired into `drivers/Makefile`: `obj-$(CONFIG_ROCKCHIP_RKNPU) += rknpu/` placed
immediately after `obj-y += gpu/` (rknpu registers a classic DRM device, so (like
`gpu/` itself) it must come after `char/` and `iommu/` per the existing comments
there, and building after `gpu/` specifically ensures the DRM core is ready first).

## Config symbols set

Via `./scripts/config --enable <SYMBOL>` then `make ARCH=arm CROSS_COMPILE=...
olddefconfig`, confirmed `=y` in `.config` afterward (no silent Kconfig-dependency
drop, all four symbols round-tripped through `olddefconfig` unchanged):

```
CONFIG_ROCKCHIP_RKNPU=y
CONFIG_ROCKCHIP_RKNPU_DRM_GEM=y
CONFIG_ROCKCHIP_RKNPU_DEBUG_FS=y
CONFIG_ROCKCHIP_RKNPU_PROC_FS=y
```

Prerequisites already satisfied pre-port: `CONFIG_DRM=y`, `CONFIG_ARCH_ROCKCHIP=y`
(both already on from the M4 display work). Left off per `PORT-PLAN.md` section 2.2/section 2.6:
`ROCKCHIP_RKNPU_DMA_HEAP`, `ROCKCHIP_RKNPU_SRAM`, `ROCKCHIP_RKNPU_FENCE`; RV1106
needs none of them (single-core NPU, no NBUF, no OPP table, no multi-domain
fencing).

## The four compat-shim headers (`drivers/rknpu/compat/soc/rockchip/`)

All four vendor-only headers from `PORT-PLAN.md` section 2.4 (absent from mainline) are
addressed, but **not uniformly**, one of the four needed a genuinely fresh shim
rather than a vendor-verbatim copy, for a reason the plan didn't anticipate:

### `rockchip_iommu.h`: written fresh, NOT a copy of the vendor header

This is the one real surprise of the port. The vendor header itself already
carries a working `#if IS_ENABLED(CONFIG_ROCKCHIP_IOMMU) ... #else <stub> #endif`
split, so naively copying it looked safe. **It is not**, in this specific tree:
`CONFIG_ROCKCHIP_IOMMU` is not a vacant symbol here: it's `=y` in our `.config`
already, for the *real*, unrelated mainline IOMMU driver
(`drivers/iommu/rockchip-iommu.c`, pulled in by the M4 display/VOP2 work). That
driver implements the standard `struct iommu_ops` and does **not** export a
function called `rockchip_iommu_is_enabled()`, confirmed by grepping
`drivers/iommu/rockchip-iommu.c` for the symbol (zero hits). Had the vendor
header been copied as-is, `IS_ENABLED(CONFIG_ROCKCHIP_IOMMU)` would have evaluated
true (not false, as the plan assumed) and selected the `extern bool
rockchip_iommu_is_enabled(struct device *dev);` declaration with **no definition
anywhere in the tree**: a link failure that would only show up at the very end of
a full kernel build, not at this file's own compile step.

Fix: `drivers/rknpu/compat/soc/rockchip/rockchip_iommu.h` is a fresh 25-line file,
unconditional (no `#if IS_ENABLED(...)` at all), providing only the one symbol
`rknpu_drv.c` actually calls: `rockchip_iommu_is_enabled()` (used once, in
`rknpu_power_off()`, inside `if (rknpu_dev->multiple_domains)`; true only for
RK3588's 3-core NPU; RV1106's `rv1106_rknpu_config` never sets it, and the board DT
carries no `iommus=` property, so this is genuinely dead code, exactly as
`PORT-PLAN.md` section 2.4 predicted: the fix just couldn't be "copy the vendor header,"
it had to be "write an unconditional one that doesn't shadow-collide with this
tree's real `CONFIG_ROCKCHIP_IOMMU`."

### `rockchip_opp_select.h`, `rockchip_system_monitor.h`, `rockchip_ipa.h`: copied verbatim, safe as-is

Unlike `ROCKCHIP_IOMMU`, none of `CONFIG_ROCKCHIP_OPP`, `CONFIG_ROCKCHIP_SYSTEM_MONITOR`,
`CONFIG_ROCKCHIP_IPA` exist anywhere in mainline (grepped every `Kconfig` in the
tree and the `.config`, zero hits for all three), so these three vendor headers'
own `#if IS_ENABLED(...)`/`#if IS_REACHABLE(...)` gates always evaluate false and
their static-inline stub branches (`-EOPNOTSUPP`/`ERR_PTR(-ENOTSUPP)`/no-ops) are
always selected, no collision risk, copied byte-for-byte from
`sdk/sysdrv/source/kernel/include/soc/rockchip/`, with only an explanatory header
comment added to each. The full struct definitions (`struct rockchip_opp_info`,
`struct monitor_dev_info`, `struct ipa_power_model_data`) are still required
unconditionally, independent of which branch is taken: `rknpu_device` (in
`rknpu_drv.h`) embeds `struct rockchip_opp_info opp_info;` **by value**, and holds
`struct monitor_dev_info *mdev_info` / `struct ipa_power_model_data *model_data`
pointers dereferenced in the (dead-for-RV1106, but still compiled) DVFS code path.

`compat/` is listed **first** in `ccflags-y` (ahead of `include/`), specifically so
these four shadow anything a future in-tree `soc/rockchip/` addition might
introduce. `rockchip_iommu.h` in particular must never resolve to a different,
unrelated header of the same name.

### Devfreq/OPP/monitor/IPA code: compiles, but is entirely unreferenced for our build

Worth recording because it looked like a real blocker mid-port and turned out not
to be one: `rknpu_drv.c` carries a large `#if KERNEL_VERSION(6,1,0) >
LINUX_VERSION_CODE / #else / #endif` split around `rknpu_devfreq_init()` (two
different implementations, one per era). For 6.18 the `#else` (>= 6.1) branch is
what's textually compiled, and it references file-scope statics (`npu_mdevp`,
`npu_devfreq_profile`, `npu_cooling_power`) that a naive manual read of the
surrounding `#if` nesting suggested might only be declared under the sibling
`< 6.1` branch. **This did not manifest as a build error**: `rknpu_drv.o` compiled
clean on the first fixed pass, which is the authoritative answer (per this task's
own "prove by compiling, not by inspection" instruction, a manual preprocessor
trace over ~700 lines of nested version gates is exactly the kind of thing to
distrust vs. the compiler). Confirmed after the fact: **none** of
`rknpu_devfreq_init`, `npu_devfreq_target`, `npu_devfreq_profile`, `npu_mdevp`, or
`npu_cooling_power` appear in `System.map`; the call site into
`rknpu_devfreq_init()` (in `rknpu_probe()`) is itself gated by the same `#if
KERNEL_VERSION(6,1,0) > LINUX_VERSION_CODE`, false for 6.18, so the whole devfreq
init path is unreachable and GCC drops the unused `static` functions entirely.
Net effect matches `PORT-PLAN.md`'s framing exactly: RV1106 has no OPP table and
no DVFS, just via straightforward dead-code elimination rather than anything this
port had to force.

## 5.10 -> 6.18 API-delta fixes (all mechanical, all found by iterating single-object builds)

### `rknpu_drv.c`

| # | Change |
|---|---|
| 1 | `struct drm_driver` has no `.gem_prime_mmap` member any more (the legacy driver-level `gem_prime_*` fallback vtable was removed). Dropped the `.gem_prime_mmap = drm_gem_prime_mmap,` initializer for the `KERNEL_VERSION(6,1,0) <= LINUX_VERSION_CODE` branch: not a functionality loss, because per-object mmap is already wired via `drm_gem_object_funcs.mmap = rknpu_gem_mmap_obj` in `rknpu_gem.c`'s `rknpu_gem_object_funcs` (the mechanism this field used to be a fallback *from*, per `PORT-PLAN.md` section 2.5). |
| 2 | `struct drm_driver` has no `.date` member any more (dropped from mainline DRM). Removed the `.date = DRIVER_DATE,` initializer; `.major`/`.minor`/`.patchlevel` already carry the version. |
| 3 | `hrtimer_init(&t, clock, mode)` + separate `t.function = fn` assignment -> combined `hrtimer_setup(&t, fn, clock, mode)` (same fix class as the RGA port's hrtimer change, `../rga/PORT-DONE.md` item 2). |
| 4 | `platform_driver.remove`: `int (*)(struct platform_device *)` -> `void (*)(struct platform_device *)`. Changed `rknpu_remove()` from `static int ... { ...; return 0; }` to `static void ...` (dropped the trailing `return 0;`), same fix as the RGA and audio ports. |
| 5 | `MODULE_IMPORT_NS(DMA_BUF)` -> `MODULE_IMPORT_NS("DMA_BUF")` (quoted-string form; same fix as the RGA port item 6). |

### `rknpu_gem.c`

| # | Change |
|---|---|
| 1 | `<linux/pfn_t.h>` and the `pfn_t` wrapper type (`__pfn_to_pfn_t()`, `PFN_DEV`) were removed entirely from mainline. Dropped the include; `vmf_insert_mixed()` (the only call site reachable at `KERNEL_VERSION(4,15,0) <= LINUX_VERSION_CODE`, which is our branch) now takes a plain `unsigned long pfn` directly: `pfn = page_to_pfn(...)` was already computing that raw value, so the fix is just passing `pfn` instead of `__pfn_to_pfn_t(pfn, PFN_DEV)`. |
| 2 | `vmap()`/`vunmap()`/`VM_MAP` used to be pulled in transitively; 6.18 needs `<linux/vmalloc.h>` included explicitly. Added it. |
| 3 | `%zu` format specifier for `rknpu_obj->size` (`-Werror=format=`): the field is declared `unsigned long`, not `size_t`, on this target, changed to `%lu`. (Pure `-Wformat` pickiness, not a real 5.10-vs-6.18 delta; the vendor's own type just doesn't match `%zu` on this ABI and newer GCC/kernel `-Werror` catches it.) |
| 4 | `iommu_map()` gained a trailing `gfp_t gfp` argument (`iommu_map(domain, iova, paddr, size, prot)` -> `iommu_map(domain, iova, paddr, size, prot, gfp)`). Added `GFP_KERNEL` at both call sites (cache-buffer path and the per-sg-entry DDR path). |
| 5 | `vma->vm_flags` is a read-only field now (direct assignment is a compile error, not just deprecated), replaced all four sites with `vm_flags_set()`/`vm_flags_clear()`: `rknpu_gem_mmap_pages()` (`VM_MIXEDMAP`), `rknpu_gem_mmap_cache()` (`VM_MIXEDMAP`), `rknpu_gem_mmap_buffer()` (`VM_DONTCOPY\|VM_DONTEXPAND\|VM_DONTDUMP\|VM_IO` set, `VM_PFNMAP` cleared). The three read-only accesses (`vm_get_page_prot(vma->vm_flags)` in `rknpu_gem_mmap`) needed no change: only assignment is blocked. |

`rknpu_reset.c`, `rknpu_iommu.c`, `rknpu_debugger.c`, `rknpu_job.c` needed **zero**
changes: compiled clean against 6.18 unmodified, confirming `PORT-PLAN.md` section 2.5's
assessment that most of the surface was already correctly version-gated by the
vendor.

## Device tree change

One board-DTS override appended to `arch/arm/boot/dts/rockchip/rv1106-warden.dts`
(base `rv1106.dtsi`'s `npu@ff660000` node: `compatible`, `reg`, `interrupts`
(`GIC_SPI 109`), `clocks` (`ACLK_RKNN`/`HCLK_RKNN`), `assigned-clock-rates
= <420000000>`, `resets` (`SRST_A_RKNN`/`SRST_H_RKNN`), left untouched, per
instructions, exactly as `PORT-PLAN.md` section 2.3 specified):

```dts
&npu {
	status = "okay";
};
```

No `iommus=` property added (non-IOMMU mode, matches the 5.10 boot-log finding);
no `power-domains` (single-rail). Verified against the **compiled** dtb (`dtc -I
dtb -O dts`, not just the source): `npu@ff660000` shows `status = "okay"`, and all
other properties resolved correctly (`interrupts = <0x00 0x6d 0x04>` = `GIC_SPI 109
IRQ_TYPE_LEVEL_HIGH`, matching the dtsi). `dtc` build output for the whole board dtb
carries exactly one warning, pre-existing and unrelated to NPU (a
`graph_endpoint`/VOP2-display bidirectionality note on `rv1106.dtsi:458`, not
touched by this port).

## Build status

Exact commands run (per the task's build recipe):

```sh
export PATH="<flare-edge>/sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin:/usr/bin:/bin"
export ARCH=arm CROSS_COMPILE=arm-rockchip830-linux-uclibcgnueabihf-
cd <flare-edge>/research/linux-6.18.46
make ARCH=arm CROSS_COMPILE=$CROSS_COMPILE zImage rockchip/rv1106-warden.dtb -j"$(nproc)"
```

Result: **exit 0**. `grep -iE "error|warn"` over the full build log returns exactly
one line: the pre-existing, NPU-unrelated dtc warning noted above. Zero errors,
zero rknpu-related warnings. `arch/arm/boot/zImage` (8.7 MB) and
`arch/arm/boot/dts/rockchip/rv1106-warden.dtb` (36.9 KB) both produced. XZ kernel
compression and the existing console/earlycon config were left untouched, per
instructions; no other in-flight work in this tree (mailbox, audio, wifi, gmac,
etc.) was reverted or altered.

## Verification: rknpu is genuinely linked in (not silently dropped to a module)

Per the task's explicit instruction, checked with **host** `grep` on `System.map`
(ground truth for built-in linkage, cross-`nm` mis-lists symbols on this
toolchain, same caveat as every other port in this series):

```
$ grep -cE 'rknpu' System.map
99
$ grep -E '\brknpu_probe\b|\brknpu_remove\b|rknpu_driver\b|rknpu_of_match|rknpu_drm_driver\b' System.map
c0be4230 t rknpu_remove
c0be476c t rknpu_probe
c177d4a0 r rknpu_of_match
c2108e58 d rknpu_driver
c2108ef0 d rknpu_drm_driver
```

99 `rknpu`-prefixed symbols are linked into the kernel image, including the
`platform_driver`'s `probe`/`remove` entry points and the `of_match_table`
(`rknpu_of_match`, confirmed at the source level to still carry the
`"rockchip,rv1106-rknpu"` compatible + `rv1106_rknpu_config` entry, untouched by
this port's fixes) and the `drm_driver` struct itself. This is the probe/of_match
symbol presence the task asked to confirm.

## Hardware test program: `rknpu_version_test.c`

Written to `warden-sdk/kernel/rv1106-enablement/npu/rknpu_version_test.c` per the
task's spec. Dependency-free beyond the kernel tree's own UAPI headers, no
libdrm, no target sysroot headers:

```sh
export PATH="<flare-edge>/sdk/tools/linux/toolchain/arm-rockchip830-linux-uclibcgnueabihf/bin:/usr/bin:/bin"
arm-rockchip830-linux-uclibcgnueabihf-gcc \
  -I<flare-edge>/research/linux-6.18.46/include/uapi \
  -Wall -O2 -static \
  -o rknpu_version_test \
  <warden-sdk>/kernel/rv1106-enablement/npu/rknpu_version_test.c
```

**This exact command was run in this session** (build-only, the resulting binary
was not copied to or executed on any target) and produced a clean ARM EABI5 static
ELF binary with exit code 0. One expected, harmless warning appears:
`#warning "Attempt to use kernel headers from user space"` from
`include/uapi/linux/types.h`, the standard notice every raw-kernel-uapi-header
userspace build gets; it does not affect correctness (`__u32` etc. are still
correctly defined with `__KERNEL__` undefined).

What it does: tries `/dev/dri/card0`, then `card1`, then `renderD128`; on the first
one that opens, issues `DRM_IOCTL_RKNPU_ACTION` with `.flags = RKNPU_GET_DRV_VERSION`
then again with `.flags = RKNPU_GET_HW_VERSION`, and prints both. Traced against the
driver source to get the exact semantics right:
- `RKNPU_GET_DRV_VERSION` returns `RKNPU_GET_DRV_VERSION_CODE(DRIVER_MAJOR,
  DRIVER_MINOR, DRIVER_PATCHLEVEL)` = `MAJOR*10000 + MINOR*100 + PATCHLEVEL`
  (`rknpu_drv.c:rknpu_get_drv_version()`), for this port's unmodified
  `DRIVER_MAJOR/MINOR/PATCHLEVEL = 0/9/2`, that's raw code `902`, which the test
  program decodes back to `"0.9.2"` via the UAPI header's own
  `RKNPU_GET_DRV_VERSION_{MAJOR,MINOR,PATCHLEVEL}()` macros.
- `RKNPU_GET_HW_VERSION` returns a raw value read directly off the NPU core's
  `VERSION`/`VERSION_NUM` registers (`rknpu_job.c:rknpu_get_hw_version()`): no
  published decode table exists for it (per `PORT-PLAN.md` section 3 step 5, "checks...
  returns something plausible"); the test program just prints it in hex.

**Expected output on a successful hardware run** (parent session):

```
opened /dev/dri/card0 (fd=3)
driver version: 0.9.2 (raw code 902)
hw version: 0x.... (raw)
PASS: /dev/dri/card0 answered both version-query ioctls -- probe, power-get/put,
and clock/reset all exercised.
```

A driver version that decodes to anything other than `0.9.2` would indicate a stale
build or a stub/mock intercepting the ioctl, not a real driver response: that's
the value of checking the decoded string, not just the ioctl return code.

## Explicitly deferred to the parent session (not done here, per this task's boundary)

- **No flashing, no boot, no `dmesg` check.** This session never touched hardware.
- Boot-time probe verification: `dmesg | grep -i rknpu` should show clean
  clock/reset/IRQ acquisition, no panic, no permanent `-EPROBE_DEFER` (a single
  deferral early at boot, before other clock/reset providers are up, would be
  normal, same caveat class as the audio port's acodec probe-order note).
  Compare against the 5.10 baseline probe log if available (`PORT-PLAN.md` section 3
  step 3).
- `ls -la /dev/dri/` should show a new `cardN`/`renderD1xx`: classic DRM node
  (this port intentionally does **not** produce a `/dev/accel/` node; see
  `PORT-PLAN.md` section 1 and `OPEN-NPU-PLAN.md` section 1.3 for why mainline's own
  `drivers/accel/rocket/` driver (RK3588/ARM64-only) is a different codebase
  that doesn't reach RV1106 at all).
- Run `rknpu_version_test` (built above) against the real node; confirm the
  decoded driver version prints `0.9.2` and the hw version is non-zero/plausible.
- **Not attempted, not required for this milestone**: `DRM_IOCTL_RKNPU_SUBMIT`,
  any `.rknn` model, `librknnrt`, that's the closed-userspace question
  `OPEN-NPU-PLAN.md` section 1.2-1.4 covers; out of scope here by design (no blob is
  shipped by this port, and none is needed to prove the kernel driver itself).

## Also updated this session

`../DRIVER-PARITY.md`'s NPU row: `[ ] M6, plan: npu/PORT-PLAN.md` -> `[wip] M6 built,
0 errors/0 warnings, 99 rknpu-prefixed symbols in System.map, &npu
{status="okay"} in the dtb, not yet flashed/probed on hardware`.
