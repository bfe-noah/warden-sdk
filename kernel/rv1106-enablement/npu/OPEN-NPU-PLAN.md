# Open NPU to 100% — plan, feasibility, and the honest ceiling

**Goal as stated:** take the RV1106 NPU to "100% — open source, verified on
hardware." **Bottom line up front:** two very different things live under that
sentence, and only one of them is reachable soon.

1. **Open, on-hardware-verified _kernel driver_** (`/dev/dri/cardN` binds, answers
   a version-query ioctl on our self-built Linux 6.18.46). **Achievable now**,
   low-single-digit engineer-days, license-clean GPL forward-port. This is the
   `PORT-PLAN.md` work in this directory — a real, bounded milestone (M6).
2. **Open _userspace_ that runs a real model** (an open compiler/encoder emitting a
   valid `regcmd` stream this driver can submit, no closed `librknnrt`, no `.rknn`
   blob). **Not achievable on any near horizon.** It is a from-scratch,
   per-SoC register reverse-engineering project with **zero existing public prior
   art for RV1106/RV1103** — the least-covered tier of the entire RKNPU family.

This document scopes both, states the most ambitious end state that is actually
achievable, and defines the concrete first milestone toward open compute.
`PORT-PLAN.md` remains the authoritative, file-by-file plan for item (1); this
document is the strategic wrapper and the item-(2) reality check. Read both.

---

## 1. Brutally honest feasibility

### 1.1 "Port the GPL kernel driver" — TRACTABLE (do it)

- The vendor driver (`flare-edge/sdk/sysdrv/source/kernel/drivers/rknpu/`, v0.9.2,
  `DRIVER_DATE 20230825`) is **SPDX GPL-2.0 at the file level**, `MODULE_LICENSE("GPL v2")`
  (`rknpu_drv.c:2176`), authored by Rockchip (Felix Zeng), mirrored at
  `github.com/airockchip/rknpu`. Legitimately open, cleanly forward-portable.
- It is a **single multi-SoC codebase that already targets RV1106 natively**:
  `of_match` entry `{ .compatible = "rockchip,rv1106-rknpu", .data = &rv1106_rknpu_config }`
  (`rknpu_drv.c:196-198`) + a dedicated config struct (`rknpu_drv.c:143-160`:
  `dma_mask=32`, `pc_data_amount_scale=2`, `pc_task_number_bits=16`,
  `pc_task_status_offset=0x3c`, single-core `irqs`/`resets` arrays,
  `nbuf_phyaddr=0/nbuf_size=0`).
- The DT node is **already fully specified** in the base tree, only disabled —
  `npu@ff660000`, `reg=<0xff660000 0x10000>`, `GIC_SPI 109`, clocks
  `ACLK_RKNN`/`HCLK_RKNN`, `assigned-clock-rates=<420000000>`, resets
  `SRST_A_RKNN`/`SRST_H_RKNN`, `status="disabled"` (`rv1106.dtsi:1127-1138`). One
  board-DTS override (`&npu { status="okay"; }`) enables it.
- **No IOMMU** on this board (`"rknpu iommu device-tree entry not found!, using
  non-iommu mode"` — CMA/contiguous DMA only), **no power-domains** (single-rail),
  **no OPP table** (fixed clock). This _shrinks_ the port: the multi-domain/DVFS/
  thermal code is all provably dead for RV1106's DT.
- The one real build blocker is four vendor-only downstream headers
  (`soc/rockchip/rockchip_{iommu,opp_select,system_monitor,ipa}.h`) absent from
  mainline 6.18 — all four back **dead-code call sites** for this SoC, fixable with
  small local compat-shim stubs (same pattern already used for `clk-rv1106.c`'s
  `panic_notifier_list` move). Full delta in `PORT-PLAN.md §2.4`.

**Verdict: real, bounded, worth doing.** Same class as the RGA port. Ends at a
clean probe + `RKNPU_GET_DRV_VERSION`/`RKNPU_GET_HW_VERSION` ioctl round-trip on
hardware — **and stops there**, because of §1.2.

### 1.2 "Open userspace that runs a model" — HARD → effectively BLOCKED for RV1106

The kernel driver's entire hardware contract is: power/clock/reset, DMA/GEM buffer
management, then it drops a **pointer + length to a userspace-authored `regcmd`
blob** into four registers and pulses "go," then fields one IRQ. **It never
inspects the regcmd contents.** (Traced through `rknpu_job.c:266-364`,
`rknpu_job_subcore_commit_pc()`.)

The four PC registers the driver programs per task (offsets into the 0xFF660000
block, from `rknpu_ioctl.h:21-39` used via `REG_WRITE` in `rknpu_job.c`):

| Offset | Name | Meaning |
|---|---|---|
| `0x00`/`0x04` | VERSION / VERSION_NUM | read-only HW version |
| `0x08` | PC_OP_EN | pulse 1→0 kicks the job; `0x1`=slave mode before commit |
| `0x10` | PC_DATA_ADDR | device address of the regcmd buffer |
| `0x14` | PC_DATA_AMOUNT | `(regcfg_amount + 4 + scale-1)/scale - 1` |
| `0x20`/`0x24`/`0x28`/`0x2c` | INT_MASK / INT_CLEAR / INT_STATUS / INT_RAW_STATUS | interrupt handshake |
| `0x30` | PC_TASK_CONTROL | `((0x6\|pingpong)<<pc_task_number_bits)\|task_number` (the `0x6` is an undecoded fixed mode field) |
| `0x34` | PC_DMA_BASE_ADDR | `task_base_addr` |

**This is the entire openly-documented register map** — the wrapper/sequencer
only. **None of it describes the compute engine** (convolution/pooling/activation
config, feature/weight DMA descriptors). Those live _inside_ the opaque regcmd
buffer. The op-code semantics of that stream:

- **Have no public register-level documentation anywhere.** TRM Part 2 does not
  exist publicly — confirmed by exhausting Rockchip doc mirrors and confirmed
  directly by a Luckfox engineer that **Luckfox itself was never given one.** Even
  Rockchip's own 90-page ROCKIVA SDK guide is pure userspace-API reference with
  zero register offsets. This is a deliberate architectural gate, not a
  hard-to-find doc.
- **Only the closed RKNN-Toolkit2 compiler emits valid regcmd streams.** The
  on-device closed `librknnrt` parses `.rknn` blobs and issues the same
  `RKNPU_MEM_*`/`RKNPU_SUBMIT` ioctls. `DRM_IOCTL_RKNPU_SUBMIT` technically accepts
  a raw command buffer directly — nothing _requires_ `librknnrt` — but since the
  buffer format is closed, that ioctl is **not** a "write your own kernel" door.

### 1.3 Why there is no shortcut through the existing open efforts

| Open effort | Chip(s) | Reaches RV1106? |
|---|---|---|
| Mainline `accel/rocket` (merged 2025-07, in our `research/linux-6.18.46/`) | RK3588 only, as shipped | **No.** `Kconfig`: `depends on (ARCH_ROCKCHIP && ARM64)`. RV1106 is 32-bit `arch/arm` — excluded before the register question. `of_match` is hardcoded `"rockchip,rk3588-rknn-core"`. |
| Mesa **Teflon** (TFLite delegate) | Only etnaviv or rocket backends | **No.** Entirely downstream of Rocket — its Rockchip reach _is_ Rocket's reach: RK3588 only. |
| `gahingwoo/linux-rk3576-npu` + `charsiu` (active, days-old as of 2026-08) | RK3576 | **No — wrong generation.** MobileNetV1 end-to-end + a Llama-3.2-1B INT4 open runtime (`charsiu`, opens `/dev/accel/accel0`, emits its own register stream) reportedly work byte-exact — but on RK3576's NPU, a **newer generation** than RV1106's. Unmerged (LKML "[PATCH v9] accel/rocket: RK3576" in review; unverifiable by second source — Anubis-walled). |
| RK3568/RK3566 Armbian fork | RK3568 | **No.** Built by byte-level diff against captured vendor command streams; multi-task still imperfect; unmerged. |
| `phhusson/rknpu-reverse-engineering` | RK3588 only, dormant since 2024-03, no license | **No.** Exploratory `strings`/`strace`/GEM-dump notes; **no compiler, no encoder, no runtime.** ~1 person-year got RK3588 to "structures identified." |
| **RV1106/RV1103** | — | **Zero hits of any kind.** No driver, no fork, no RE writeup, no captured traces. Least-covered tier in the family. |

RV1106 sits in the RKNPU2 **"v1" family** (RK3566/68, RV1109/26, RV1103/06 —
INT8-only, `quantize=8` mandatory). RK3576/RK3588 are a **different, newer**
generation. Precedent (RK3568 needing byte-level RE off RK3588) shows this is a
**bespoke per-SoC RE project even between adjacent generations, not a recompile.**

### 1.4 The most ambitious _achievable_ end state

Ranked by realism:

- **Tier A — ACHIEVABLE NOW (commit to this):** open GPL kernel driver, statically
  built into our 6.18.46, `/dev/dri/cardN` binds, `/proc/rknpu/load` live for the
  Monitor page, `RKNPU_GET_HW_VERSION`/`RKNPU_GET_DRV_VERSION` verified on
  hardware. **No blob shipped.** This is item (1); it is the honest "100% open" for
  the _driver_.
- **Tier B — ACHIEVABLE AS A RESEARCH SPIKE, bounded, low-value:** a **single
  hand-written op** (one conv or one matmul) executed via a `regcmd` **captured**
  from the closed stack and **replayed** through our open `DRM_IOCTL_RKNPU_SUBMIT`,
  byte-for-byte, with an open host tool — proving the open submit path can drive
  real compute. This does **not** require understanding the regcmd ISA; it requires
  capturing one. It is the concrete first milestone toward open compute (§3, M-NPU-2)
  and the only way to get an on-hardware compute number without the RE project.
- **Tier C — NOT ACHIEVABLE on any near horizon:** an open **compiler/encoder** that
  emits novel regcmd streams for arbitrary models. This is the ~person-year+
  from-scratch RE project (§3, M-NPU-3+). We should **scope it, not staff it**,
  unless a strategic reason to own open NPU compute emerges.

**Honest recommendation:** ship Tier A. Do Tier B only as a time-boxed spike _if_ a
real non-visual inference use case (audio/RS-485/touch-gesture classifier — the
only sensible NPU use here; no camera exists on this board) is actually wanted.
Treat Tier C as closed-vendor-runtime-only for the foreseeable future. This matches
the standing `PROVENANCE.md` policy: **we do not ship the closed runtime blob**;
the open path is a from-scratch regcmd encoder tracked separately, never a vendored
binary — and until that encoder exists, open NPU _compute_ does not exist for us.

---

## 2. Key technical facts (for the implementer)

### Register / memory map (TRM Part1 — Part2 does not exist publicly)

| Block | Base | Size |
|---|---|---|
| NPU core | `0xFF660000` | 64KB (`reg=<0xff660000 0x10000>`) |
| NPU_GRF | `0xFF018000` | 32KB |
| NPU_SGRF | `0xFF074000` | 8KB |
| NPU_CRU | `0xFF3B6000` | 8KB |
| NPU_CBUF (scratch SRAM) | `0xFF680000` | 256KB (unused: config sets `nbuf=0`) |

IRQ `GIC_SPI 109 LEVEL_HIGH`, node `ff660000.npu`. Clocks `ACLK_RKNN`/`HCLK_RKNN`
off the CRU matrix; only measured clock point anywhere is **NPU 500MHz** (Rockchip
internal power-test xlsx, "typical IPC workload" corner). A commented-out board-DTS
override would set 700MHz — never applied.

### UAPI (the seam an open userspace must target)

Two memory-manager paths, Kconfig-selected. **RV1106's shipping defconfig picks
`CONFIG_ROCKCHIP_RKNPU_DMA_HEAP=y`** (`/dev/rknpu` misc device, bare `IOCTL_RKNPU_*`
codes, magic `'r'`) — but `PORT-PLAN.md` deliberately chooses **DRM_GEM** for the
6.18 port (`/dev/dri/cardN`, `DRM_IOCTL_RKNPU_*`, `DRM_RENDER_ALLOW`), because M4
already pulls DRM in for the panel and DRM_GEM is the vendor default. Same struct
payloads either way. Six ioctls: `RKNPU_ACTION`, `RKNPU_SUBMIT`,
`RKNPU_MEM_{CREATE,MAP,DESTROY,SYNC}` (`rknpu_ioctl.h:288-322`).

```c
struct rknpu_task {                 // rknpu_ioctl.h:218-228
    __u32 flags, op_idx, enable_mask;
    __u32 int_mask, int_clear, int_status;
    __u32 regcfg_amount, regcfg_offset;
    __u64 regcmd_addr;              // <-- points at the OPAQUE compute program
} __packed;
struct rknpu_submit {               // rknpu_ioctl.h:260-274
    __u32 flags, timeout, task_start, task_number, task_counter;
    __s32 priority;
    __u64 task_obj_addr, regcfg_obj_addr, task_base_addr, user_data;
    __u32 core_mask; __s32 fence_fd;
    struct rknpu_subcore_task subcore_task[5];
};
```

An open userspace must produce: **(a) the `struct rknpu_task[]` list — fully
inferable from the kernel driver alone, trivial;** and **(b) the regcmd byte stream
each task points to — NOT inferable from the driver or any published TRM.** (b) is
the whole problem.

### `/proc/rknpu` — safety (hardware-confirmed)

- `/proc/rknpu/load` — **safe** (50 clean reads verified); the Monitor page polls it.
- `/proc/rknpu/freq`, `/power` — exposed, unverified.
- `/proc/rknpu/volt` — **CONFIRMED SIGSEGV** (null deref — no regulator wired). Never poll.

### Provenance / policy

`PROVENANCE.md`: the kernel driver is portable GPL; the closed piece is the
userspace RKNN runtime + regcmd format (a blob). **Per directive we do not ship
that blob.** `CAPABILITIES-AUDIT.md:32` rates NPU "not worth shipping" until an
open encoder exists.

### URLs

- Vendor driver: `github.com/airockchip/rknpu`, `github.com/rockchip-linux/rknpu`
- Mainline Rocket: `drivers/accel/rocket/` (RK3588-only); blog:
  `blog.tomeuvizoso.net/2025/07/rockchip-npu-update-6-we-are-in-mainline.html`
- Teflon: `docs.mesa3d.org/teflon.html`; NLnet: `nlnet.nl/project/Rockchip-NPU-driver/`
- RK3576 RE: `github.com/gahingwoo/{linux-rk3576-npu,charsiu,kiln}` (active, unmerged)
- RK3588 RE: `github.com/phhusson/rknpu-reverse-engineering` (dormant, no license)
- Official closed docs (RV1106/RV1103 quick-start PDFs, API-only): `github.com/airockchip/rknn-toolkit2` `doc/`

---

## 3. Milestones

### M-NPU-1 — Open kernel driver, verified on hardware (ACHIEVABLE — the real deliverable)

Follows `PORT-PLAN.md` in full. Summary of the gates:

1. **Build**: `CONFIG_ROCKCHIP_RKNPU=y` + `_DRM_GEM=y` + `_DEBUG_FS`/`_PROC_FS`;
   compile `rknpu_{drv,job,gem,reset,iommu,debugger}.o` clean into `built-in.a`.
   Prove the four §2.4 compat shims (`compat/soc/rockchip/rockchip_{iommu,opp_select,
   system_monitor,ipa}.h`) against real 6.18 headers — this is where they get
   verified, not just inspected.
2. **DT**: board override `&npu { status="okay"; }`; `dtc -W` clean; re-check the
   "place the fdt high" DTB-placement fix (bigger `built-in.a`).
3. **Boot** via the `warden-c8a3` `_b`-slot one-shot loop (never touches `_a`,
   auto-reverts on hang): `dmesg | grep -i rknpu` shows clean probe — clock/reset/
   IRQ acquired, no panic, no `-EPROBE_DEFER` stall.
4. **Node**: `/dev/dri/cardN` (or `renderD1xx`) appears — classic DRM node, **not**
   `/dev/accel/`.
5. **Verify ioctl**: host-buildable C program opens the node, issues
   `DRM_IOCTL_RKNPU_ACTION {.flags=RKNPU_GET_DRV_VERSION}`, checks `value` decodes
   to `0.9.2`; `RKNPU_GET_HW_VERSION` returns something plausible. Exercises full
   dispatch → power-get/put → clock/reset with no regcmd dependency.
6. **Capture the durable delta** as a patch series in this dir (compat shims + DT
   fragment + config fragment); vendor source stays in `research/linux-6.18.46/`.

**Effort:** low-single-digit engineer-days. **Risk:** a surprise vendor-only symbol
not surfaced by the read-through; DTB placement. **This is "NPU driver = 100% open,
verified."** Done here for the driver half.

### M-NPU-2 — First open compute: capture-and-replay one op (SPIKE, only if wanted)

The concrete first milestone toward open _compute_, and the honest Tier-B ceiling.
It does **not** require decoding the ISA.

1. On a unit with the closed stack available (build host / a dev panel with the
   closed `librknnrt` + a trivial single-conv or single-matmul `.rknn`), capture
   the regcmd buffer(s) the runtime DMAs in — via `strace` of the `RKNPU_MEM_*`/
   `RKNPU_SUBMIT` ioctls + `GEM_FLINK`/`GEM_OPEN` buffer dumps (phhusson's method),
   plus the input/output tensor buffers.
2. Write an **open host tool** that: allocates the same GEM buffers via
   `RKNPU_MEM_CREATE`, writes the captured regcmd bytes + captured input tensor,
   builds the `struct rknpu_task[]` (fully open, §2), and submits via
   `DRM_IOCTL_RKNPU_SUBMIT` on our M-NPU-1 driver.
3. **Verify**: output tensor matches the closed runtime's output **byte-for-byte**
   on hardware, and matches a CPU/NEON reference for the same op. Record the
   **measured job-submission latency** (ioctl+DMA+blocking-IRQ) — a number that
   exists nowhere today and settles the "NPU vs NEON dispatch overhead" argument.

**Deliverable:** proof the open submit path drives real compute + first latency
number. **Not** a general runtime — the regcmd is a fixed captured blob for one op
shape. **Value:** low; do only as a time-boxed research spike behind a real
use case. **Never ship a captured Rockchip regcmd as a product artifact** (it is
their compiler's output) — this stays in `research/`.

### M-NPU-3+ — Open regcmd encoder (SCOPE ONLY — do not staff without a strategic reason)

The Tier-C from-scratch RE project. Method (the only demonstrated one):
differentially decode captured regcmd streams across many hand-built minimal ONNX
models (isolate one op/param at a time), reconstruct the compute-engine register
semantics, build an encoder for a first op, then grow the op set. Precedent cost:
~1 person-year got RK3588 to "exploratory, no compiler"; RK3568 needed byte-level
diffing and still isn't merged; **RV1106 starts from literal zero.** Track here if
ever begun; otherwise this is the documented reason open NPU compute is deferred.

---

## 4. Risks / open questions

- **The core risk is misframing.** "The driver is open source" must not be sold as
  "the NPU is open-source-usable." M-NPU-1 delivers an open driver that can submit
  jobs; it delivers **no open way to produce a valid job.** Guard this in every
  status line.
- Additional vendor-only symbols beyond the four headers may surface only at
  compile — M-NPU-1 gate 1 is the real test.
- **SKU unconfirmed** (G2 128MB / 0.5-TOPS vs G3 256MB / 1.0-TOPS; datasheet Rev 2.0
  retracted the split to "both 1.0 TOPS"). Determines the compute ceiling if
  M-NPU-2/3 are ever pursued; needs an on-hardware chip-ID/TOPS register read.
- **No open regcmd project for RV1106 exists** (re-confirm periodically — community
  moves fast, esp. the `gahingwoo` RK3576 work; but it is the wrong generation).
- **No NPU use case is currently established for this board** (no camera; RGA2 +
  RKIVE already cover the 2D/CNN-shaped image tasks better — see
  `npu-graphics-feasibility.md`). Absent a concrete non-visual classifier need,
  M-NPU-2 and beyond have no pull, and M-NPU-1 alone (driver binds, load graph
  works, no blob) is the correct stopping point.

---
_Cross-refs: `PORT-PLAN.md` (authoritative file-by-file kernel port),
`../../docs/npu-graphics-feasibility.md`, `../CAPABILITIES-AUDIT.md:32`,
`../PROVENANCE.md`, `../DRIVER-PARITY.md:41`, `../REMAINING-PORTS.md §6`._
