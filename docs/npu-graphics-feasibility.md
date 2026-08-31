# NPU Graphics Feasibility

> Point-in-time engineering study, written while scoping the `rknpu.ko` 6.18
> port for the downstream WardenOS firmware; "our boards" / "this product"
> below refer to that context. The hardware conclusions apply to any 86 Panel.

**Bottom line: no, not for 3D rendering; not "slower," but genuinely not how the
hardware works past the first pipeline stage. The RKNPU on RV1106 is a
fixed-function, INT8-only, command-stream tensor accelerator with no rasterizer,
no texture unit, no per-pixel programmability, and no framebuffer output; only
the vertex-transform stage of a 3D pipeline is even shape-compatible with what it
does (matmul), and at this panel's UI scale that alone isn't worth the dispatch
overhead. A few CNN-shaped image-processing tricks (blur, sharpen, edge
detection, super-resolution) are theoretically expressible on it, but this
product already has two better-fitting, cheaper, already-proven accelerators for
that territory (RGA for 2D ops, RKIVE for classic CV filters) and no camera to
feed a vision model in the first place. Written ahead of the RKNPU driver's
Linux 6.18 port so the port is scoped honestly: port it (if at all) for future
non-visual inference, not for graphics.**

This document answers a single question raised while planning that port: given
we're about to carry `rknpu.ko` forward to a new kernel, could the NPU pull any
graphics weight on a GPU-less SoC? Facts are cited to their source: the
hardware wiki (`luckfox-pico-86-panel/`), the product wiki
(`flare-edge-construction/`), the vendored SDK kernel driver source
(`flare-edge/sdk/sysdrv/source/kernel/drivers/rknpu/`), the RV1106 devicetree,
or flagged as general knowledge / needing TRM confirmation.

---

## 1. What the NPU Is

### Identity and Generation

- It is the **RKNPU**, Rockchip's 4th-generation NPU IP, exposed to tooling as
  the "RKNPU2" software generation (same toolchain family as RK3566/68/88), but
  RV1103/RV1106 sit in that family's **INT8-only tier**: RKNN-Toolkit2 conversion
  requires `quantize=8`, with no mixed-precision or FP path available on this
  silicon, unlike the larger RK356x/RK3588 SKUs that also support it.
  (`luckfox-pico-86-panel/npu.md:7`, `soc-rv1106.md`)
- The chip's own datasheet lists it as supporting mixed **INT4/INT8/INT16**
  precision at the IP-family level, and states it "supports creating simple
  custom operators" (the RKNN custom-op extension), but this is an extension
  mechanism for adding a new *operator* to the compiled-graph model, not general
  programmability (see below). (`luckfox-pico-86-panel/npu.md:7`,
  `raw/web-camera-isp-npu.md:9`)
- **Throughput (TOPS) is a genuinely unsettled number, not a fact to hard-code.**
  Rockchip's own datasheet rated G2=0.5 TOPS / G3=1.0 TOPS from Rev 1.2
  (2022-12-12) through Rev 1.9 (2025-12-12); Rev 2.0 (2026-04-02) retracted this
  and now states both G2 and G3 = 1.0 TOPS ("Correct NPU performance as 1 TOPS,"
  no benchmark given). Luckfox's own live product wiki still publishes the
  *older* 0.5/1.0 split. Which figure applies to a given 86-Panel unit also
  depends on its SKU (0208/0408 = G2, 1208/1408 = G3), which isn't confirmed for
  our boards. **Any TOPS number in this document should be read as "half to one
  TOPS, disputed", not a precise spec.** (`luckfox-pico-86-panel/npu.md:8`,
  `soc-rv1106.md:31-45`)
- One real measured clock point exists at all: a leaked-but-mirrored Rockchip
  internal power-test report states a "typical IPC workload" corner of **NPU
  500MHz**; the 86-Panel's own (commented-out) DTS NPU clock stanza instead
  assigns **420MHz**. No datasheet states a default/rated frequency.
  (`luckfox-pico-86-panel/npu.md:11-13`)

### Architecture

Everything downstream rests on this fact. Reading the vendored
kernel driver source directly (`flare-edge/sdk/sysdrv/source/kernel/drivers/rknpu/`):

- The driver's register offsets (`rknpu_ioctl.h`) center on a **"PC" (program
  counter) task-list model**: `RKNPU_OFFSET_PC_OP_EN`, `PC_DATA_ADDR`,
  `PC_DATA_AMOUNT`, `PC_TASK_CONTROL`, `PC_DMA_BASE_ADDR`. Userspace builds a
  **register command list** (`regcmd`) describing a sequence of hardware
  operations, DMAs it into the device via `struct rknpu_task` /
  `struct rknpu_submit` (`regcfg_obj_addr`, `regcmd_addr`, `task_obj_addr`), and
  the hardware executes that command stream to completion, raising an interrupt
  the driver waits on (`wait_event_timeout(...msecs_to_jiffies(args->timeout))`
  in `rknpu_job.c`). This is **not** a shader core fetching and executing
  arbitrary instructions per invocation; it's closer to a DMA-fed
  fixed-function pipeline being told "run this pre-built op sequence over these
  buffers."
- **The actual op-code semantics of that regcmd stream are not publicly
  documented.** The hardware wiki is explicit: register-level detail (TRM Part 2)
  "does not exist publicly, confirmed by exhausting all known Rockchip doc
  mirrors," including Rockchip's own NPU SDK guide, which is "pure userspace-API
  reference with zero register offsets." NPU access on this SoC is
  "architecturally gated behind the RKNN userspace API stack, not just
  under-documented." (`luckfox-pico-86-panel/npu.md:14`) This means: **no one
  outside Rockchip's compiler team can hand-write a regcmd stream that does
  something novel** (e.g., a rasterizer); only the closed RKNN-Toolkit2
  compiler emits valid ones, by lowering a supported ONNX graph (conv, pool,
  elementwise, activation, and similar tensor ops) into that command format.
  What "operators" the compiler can lower is itself the real ISA surface, and it
  is a CNN operator set, not a general instruction set. *(Flag: this document's
  claim that no rasterization/gather/sampling primitive exists in the regcmd ISA
  is reasoned from the RKNN operator taxonomy and general 4th-gen RKNPU
  architecture, not from register-level ground truth; the TRM that would settle
  it definitively does not exist publicly. Treat as high-confidence, not
  certain.)*
- **Is there a lower-level submit path than the RKNN runtime?** Technically yes:
  `DRM_IOCTL_RKNPU_SUBMIT` / `IOCTL_RKNPU_SUBMIT` accepts a raw
  `regcfg_obj_addr`/`regcmd_addr` task list directly; nothing in the kernel
  driver *requires* going through `librknnrt`. But this is the same interface
  the RKNN runtime itself calls internally; the driver has no knowledge of
  "operators" at all, only "a command buffer and some memory handles." Since the
  command-buffer format is closed, this ioctl is not a usable "write your own
  compute kernel" door for us; it's an implementation detail of the vendor
  runtime we'd be building on top of via the RKNN C API regardless.
- Custom operators (per the datasheet's "creating simple custom operators")
  extend the *model graph* with a new node type, still compiled by the RKNN
  toolchain into the same regcmd format, still constrained to whatever
  primitive operations the hardware's fixed-function units implement
  underneath. It is not a route to arbitrary per-element or per-pixel code.

### Data Types and Memory

- **INT8-only quantization tier** for RV1106/RV1103 (`quantize=8` mandatory at
  conversion time); inputs/outputs must be int8 and strictly 4-D.
  LayerNormalization and ReduceL2 aren't supported by the RKNN parser at all and
  must run on the Cortex-A7 before/after the NPU call.
  (`luckfox-pico-86-panel/npu.md:35`, `raw/web-camera-isp-npu.md:43`)
- **No dedicated VRAM.** The NPU shares the same in-package DDR3L as everything
  else: 128MB (G2) or 256MB (G3) total, shared with Linux, any RGA/ISP buffers,
  and NPU weights/activations. There is a small on-chip scratch: `NPU_CBUF`
  256KB SRAM plus an optional SRAM/NBUF allocation path in the driver
  (`rknpu_gem.c`, `RKNPU_MEM_TRY_ALLOC_SRAM`/`_NBUF`), a cache for
  weights/activations, not a general framebuffer-sized memory.
  (`luckfox-pico-86-panel/npu.md:10`, `soc-rv1106.md:47-98`)
- **No IOMMU wired on this board.** Boot log shows `"rknpu iommu device-tree
  entry not found!, using non-iommu mode"`; every buffer the NPU touches today
  must be physically-contiguous DMA memory, the same constraint RGA has on this
  chip. (`luckfox-pico-86-panel/npu.md:26`)
- **Real-world binding constraint is RAM, not TOPS**: forum/GitHub evidence
  shows YOLOv5s running but YOLOv8/YOLOv10 hitting memory errors on RV1106;
  the 128-256MB shared-DDR budget is the practical ceiling well before compute
  is. (`luckfox-pico-86-panel/npu.md:38`)

### Software Stack

1. **Kernel driver** (`rknpu.ko`, currently v0.9.2 on our shipped firmware):
   exposes `/dev/rknpu` (a DRM device or a misc device, selectable at build time
   via `ROCKCHIP_RKNPU_DRM_GEM` vs `ROCKCHIP_RKNPU_DMA_HEAP`, see
   `drivers/rknpu/Kconfig`), handles memory allocation (`RKNPU_MEM_CREATE` /
   `_MAP` / `_DESTROY` / `_SYNC` ioctls), job submission
   (`RKNPU_SUBMIT`), and misc actions (frequency/voltage/power, bandwidth
   priority; `RKNPU_ACTION` ioctl enum in `rknpu_ioctl.h`).
2. **RKNN userspace runtime** (on-device C API, the *only* supported on-target
   API on RV1106/RV1103; the Python API is PC-side verification only): `rknn_init()`
   -> `rknn_query()` -> `rknn_create_mem()` -> `rknn_set_io_mem()` -> `memcpy()` input
   -> `rknn_run()` -> dequantize `float = (int_output - zero_point) * scale`.
   (`luckfox-pico-86-panel/npu.md:34`)
3. **RKNN-Toolkit2** (PC-side, x86 only, Python, Ubuntu-only wheels): the
   offline compiler: train (PyTorch/TF) -> export ONNX -> convert/quantize to a
   `.rknn` file. This step is where the regcmd command stream actually gets
   generated; it happens once, offline, not per-frame. (`luckfox-pico-86-panel/npu.md:33`)

**On our product today**: `rknpu.ko` is loaded (`S24npu` init script) and
`/proc/rknpu/load` is polled purely to drive the Monitor page's NPU-load
graph; there is no evidence anywhere in the platform wiki's raw notes of an
actual `rknn_init()`/`rknn_run()` inference workload ever having been run on
this product. The NPU is live and idle from a compute standpoint.
(`luckfox-pico-86-panel/npu.md:28`) `/proc/rknpu/volt` is a confirmed SIGSEGV
footgun on this board (no regulator wired): never poll it.
(`luckfox-pico-86-panel/npu.md:27`)

---

## 2. 3D Rendering, Stage by Stage

A conventional 3D pipeline: **vertex transform -> primitive assembly ->
rasterization -> depth test -> texture sampling -> per-pixel shading ->
framebuffer write.** Verdict per stage, given everything in section 1:

| Stage | Maps to NPU? | Why / how |
|---|---|---|
| **Vertex transform** (MVP matrix x vertices) | **Yes, in principle** | This is exactly GEMM/matmul, the NPU's actual strength as a tensor accelerator. A batch of vertices as an input tensor, a weight-like MVP matrix, one matmul op. This is the *only* stage that's hardware-shape-compatible. |
| **Primitive assembly** (grouping vertices into triangles) | **No** | Not a tensor op at all; it's index-buffer bookkeeping/control flow. Trivial on a CPU, meaningless to express as a conv/pool/elementwise graph. |
| **Rasterization** (triangle scan-conversion, edge functions, coverage) | **No, not how the hardware works** | There is no scan-conversion primitive in the RKNN operator set or (as far as the undocumented regcmd ISA can be inferred) the hardware's fixed-function units. A CNN accelerator computes dense/windowed reductions over a tensor; it has no per-primitive geometric test. This is not "slow," it's absent. |
| **Depth test** (per-pixel z-buffer compare-and-write) | **No** | Requires a read-compare-conditional-write per pixel against arbitrary prior state; not an operation in the conv/pool/elementwise/activation vocabulary, and there's no depth-buffer-shaped hardware resource on this IP. |
| **Texture sampling** (bilinear/nearest fetch from an image by UV) | **No (high confidence, needs TRM to fully settle)** | No "gather/sample" op appears anywhere in the documented RKNN operator taxonomy for this tier. Convolution can *read* a spatial neighborhood, but that's not the same primitive as an arbitrary-address texture fetch with wrapping/filtering. |
| **Per-pixel shading** (arbitrary per-fragment program) | **No** | The NPU executes one fixed, precompiled graph over a whole tensor; it cannot run per-pixel conditional/arbitrary code. You could contrive a *specific* visual effect that literally is a small CNN (see section 3), but that's not "shading" in the pipeline sense; it's a different, narrower thing wearing the name. |
| **Framebuffer write** (write final pixels to the display's scanout buffer) | **No** | The NPU has no display/scanout connection at all: no DRM plane, no VOP link. Its only output path is writing tensor data to a DDR buffer, which is not a display pixel format. Something else (CPU or RGA) has to dequantize (`int8 -> float -> pixel`) and repack it into an actual framebuffer format before it's visible, and even RGA doesn't consume NPU tensor layouts directly (see section 4). |

### Verdict

**No full pipeline is possible on this hardware: five of six stages have no
mapping at all, not a slow one.** A "hybrid" design where only vertex transform
runs on the NPU and everything else (rasterize, depth-test, texture, shade,
write) runs on the Cortex-A7 is the only thing even worth evaluating, and it
doesn't clear the bar either:

- **Dispatch cost is real and not free.** Submission goes through an ioctl,
  a DMA of the command/data buffers, and a **blocking wait on a hardware
  interrupt** (`wait_event_timeout` in `rknpu_job.c`); this is a job-queue
  round trip through the kernel, not a same-cycle instruction. No on-hardware
  latency number exists in the wiki or SDK for this board (flagged as needing
  measurement, not asserted here), but the *shape* of the interface (ioctl +
  DMA + IRQ wait) is categorically heavier than a same-thread function call.
- **The scale doesn't justify it.** This is a 720x720 wall-panel UI rendering
  simple chrome, not a game engine; any "3D" element (an isometric icon, a
  rotating gauge) involves a handful to a few hundred vertices per frame. A
  Cortex-A7 with NEON does a few-hundred-vertex 4x4 matrix transform in low
  single-digit microseconds; there is no plausible world where paying an
  ioctl/DMA/IRQ round trip to a shared-DDR accelerator beats that, even before
  accounting for the INT8 quantization step (packing/unpacking float vertex and
  matrix data to/from int8 with scale/zero-point, and the *dynamic range*
  problem: an MVP matrix spans near-to-far-plane depth ranges that quantize
  very poorly to INT8 without per-frame requantization).
- **The offline-compile step doesn't fit a live camera-driven UI anyway.** RKNN
  models are compiled ahead-of-time by RKNN-Toolkit2 on a PC; while the runtime
  *can* accept different input tensor values per `rknn_run()` call (so a
  "run this fixed matmul graph on today's MVP matrix" model is technically
  legitimate), any change to the graph shape itself is a PC-side recompile, not
  a runtime option, a real constraint for anything beyond the most rigid,
  pre-planned use of the matmul stage.

**Net: don't chase this.** Even the one stage that's shape-compatible isn't a
net win at this UI's scale, and the other five stages are not partial-credit;
they are the wrong tool, full stop.

---

## 3. CNN-Shaped Image Tasks

Setting 3D aside: a CNN accelerator's real strength is convolution, which
*does* map to some classic image-processing tasks. Evaluated against this
specific 0.5-1 TOPS-class, 128-256MB-shared-DDR, no-camera product:

| Task | Technically fits an NPU? | Realistic on this product? |
|---|---|---|
| **Blur / sharpen / edge detection** (convolution kernels) | Yes; this is literally what conv2d does | **No; RKIVE already exists for this, and is a better fit.** RKIVE (Rockchip Intelligent Video Engine) is a *separate*, fixed-function classic-CV block at `0xFFAD0000` (Canny edge, Sobel, morphology (erode/dilate), histogram, connected-components, optical flow, block matching) that sits **completely idle** on this board today, needs no model-compile step, and is architecturally the intended hardware for exactly this class of filter. (`soc-rv1106.md:108`, `raw/web-camera-isp-npu.md:13`) Reaching for the NPU (compile a model, quantize INT8, pay job-submit overhead) to do a blur that RKIVE or even plain CPU already does more directly is solving an already-solved problem the hard way. This product also has direct, recorded evidence that *any* blur is expensive without a GPU: a 42px soft "flare" shadow effect measurably tanked LVGL's DRM-backend performance and was replaced with a cheap 2px border (`flare-edge-construction/design-system.md:22`); the fix that shipped was "don't blur," not "blur on a different accelerator." |
| **Super-resolution / upscaling** | Yes; small SR CNNs (ESPCN-class) exist and run on comparable RKNPU2-family chips | **No use case.** The panel renders its own UI natively at its native 720x720 resolution; there is no lower-resolution source content needing upscaling, and no camera feed to upscale (the 86-Panel has zero camera hardware, confirmed by schematic inspection, `luckfox-pico-86-panel/modernization-roadmap.md:76`). Dead on arrival for lack of an input, not for lack of hardware capability. |
| **Style transfer** | Yes; it literally is a CNN | **No, oversized and pointless.** Fast-neural-style-class networks are typically larger than YOLOv5s, which is already near this board's practical RAM ceiling (YOLOv8/v10 already error out on RV1106's shared DDR, `npu.md:38`). There's also no product need for a stylized-UI-render feature. |
| **Segmentation-driven UI effects** | Yes, in principle | **Moot: no camera, no visual input of any kind to segment.** |
| **2D affine transforms** (rotate/scale/skew as matrix math) | Yes, technically a small matmul | **No; RGA already does this natively, in fixed-function hardware, cheaper.** RGA2-Enhance on this board already does scale (bicubic up / averaging down, to 16x either direction), rotate (90/180/270° on input windows), crop, and color/format conversion as dedicated blit-engine operations: no model compile, no INT8 quantization, no job-submit-and-IRQ-wait round trip, just a register-programmed blit. It is already wired into LVGL (the Monitor-page double-buffer-sync offload, verified 20%->8% CPU on real hardware) and proven in production. (`luckfox-pico-86-panel/rga.md`) |

### Prior Rejection

The product wiki records that the keyboard's touch-bias correction (snapping an
ambiguous tap to the nearest key) was **explicitly evaluated for NPU
acceleration and not taken**: the team asked "Is the key bias implementation
feasible using the NPU on this device?" and shipped plain nearest-key-rectangle
geometry instead, using LVGL's own buttonmatrix internals.
(`flare-edge-construction/design-system.md:73`) That is exactly the right call
for the reasons in this document: a tiny, cheap, well-defined 2D geometric
problem has no business going through a tensor accelerator's compile-and-submit
pipeline. Nothing in this research changes that conclusion; if anything it
generalizes it.

### Realistic Verdict

None of the CNN-shaped graphical tasks clear the bar for this specific product.
Where a hardware assist genuinely helps (2D blit/scale/rotate/blend, classic
CV filters), this SoC already has two purpose-built, cheaper, proven-or-idle
accelerators (RGA, RKIVE) that are the architecturally correct answer, not the
NPU. The NPU's actual realistic value on this product remains what the platform
wiki already concluded independent of this research: **small, non-visual
inference** (audio classification off the on-die codec, RS-485/sensor anomaly
detection, touch-gesture-pattern classification), not graphics of any kind.
(`luckfox-pico-86-panel/npu.md:46-50`)

---

## 4. Driver Porting

### Porting Scope

- **This is a forward-port of Rockchip's out-of-tree vendor driver, not a
  from-scratch write.** The driver already carries version-gated compatibility
  shims for kernel APIs that changed across versions, e.g. `rknpu_iommu.c` has
  `#if KERNEL_VERSION(6, 1, 0) > LINUX_VERSION_CODE` / `#if KERNEL_VERSION(5, 10, 0)
  <= LINUX_VERSION_CODE` branches for IOVA/dma_limit API differences, showing
  Rockchip's own driver source is written to track multiple kernel versions,
  which is a good sign for portability in principle but confirms real API-level
  work is needed, not a recompile.
- **No mainline path exists to lean on.** The open-source "Rocket" NPU driver
  (`accel/rocket`) covers RK3588 and (as of a 2026-07-15 reverse-engineering
  effort) RK3576, but RV1106's 4th-generation NPU IP is a **different
  generation** and is not covered by Rocket, and neither active mainline RV1106
  patch series (Simon Glass's SoC/clk/pinctrl series, Vladislav Leonov's
  peripheral series) touches NPU, RGA, ISP, VENC, or display at all.
  (`luckfox-pico-86-panel/mainline-kernel.md:38`, `modernization-roadmap.md:48,96`)
  **NPU use on this chip requires the proprietary RKNPU2 vendor runtime
  indefinitely**: there is no future where an open driver + open compiler
  replaces it.
- **Memory manager choice matters for the port.** The driver's Kconfig offers
  two mutually exclusive memory managers: `ROCKCHIP_RKNPU_DRM_GEM` (needs the
  DRM subsystem; DRM GEM/fence APIs have moved substantially between 5.10 and
  6.18) or `ROCKCHIP_RKNPU_DMA_HEAP` (needs `DMABUF_HEAPS_ROCKCHIP_CMA_HEAP`).
  Whichever is chosen inherits whatever DRM/dma-buf/dma-fence API churn exists
  across that kernel gap, the same class of surface RGA's port would also have
  to cross.
- **IOMMU status is a live design choice, not a given.** Today this board runs
  the NPU in **non-IOMMU mode** (no DT entry), same physically-contiguous-only
  memory constraint RGA has on this chip. The kernel driver does support an
  IOMMU path (`rknpu_iommu.c`), so wiring it up is possible but is new scope,
  not something the port inherits for free.
- **Reset/clock plumbing**: `SRST_A_RKNN`/`SRST_H_RKNN` resets and
  `ACLK_RKNN`/`HCLK_RKNN` clocks off the shared CRU "matrix" clock ladder, node
  `npu@ff660000` in `rv1106.dtsi` (`status = "disabled"` at the base dtsi level;
  our board enables it downstream); unremarkable, same pattern as every other
  RV1106 peripheral node.
- **Bottom line on effort class**: this is the same class of work already
  scoped for RGA in the modernization roadmap ("carrying Rockchip's out-of-tree
  driver forward against a newer kernel ABI"): bounded, evidence-backed, but
  real engineering, not a version-string bump. (`luckfox-pico-86-panel/rga.md:86`)

### Direct Submit Path

**It needs the full toolchain.** As established in section 1, the raw
`DRM_IOCTL_RKNPU_SUBMIT` path exists at the kernel-ioctl level, but the
command-buffer format it consumes is generated exclusively by the closed
RKNN-Toolkit2 compiler and is not publicly documented at the register level.
There is no supported "hand-roll a compute kernel" door here; any real
workload (an inference model, or a hypothetical matmul-as-graphics use) has to
go: train/define -> ONNX -> RKNN-Toolkit2 compile (PC, offline) -> ship the
`.rknn` blob -> RKNN C API (`rknn_init`/`rknn_run`) on-device. This is a heavier,
slower-to-iterate loop than driving RGA (which is a direct, synchronous
`im2d`-style C API call with no offline compile step at all) or writing plain
CPU code.

### RGA Comparison

Worth restating plainly since it's the thing the NPU would be compared against:
**RGA2-Enhance already does everything this panel's UI plausibly needs from 2D
hardware acceleration**: blit/copy, scale (bicubic, up to 16x), rotate
(90/180/270° on input), full CSC (BT601/BT709), blend (Porter-Duff), colorkey,
ROP, fill, dither, mosaic, and a purpose-built OSD compositing path, and it's
already integrated into LVGL with a measured, shipped production win (Monitor
page 20%->8% CPU). (`luckfox-pico-86-panel/rga.md`) There is no 2D graphics gap
on this product that would motivate reaching for the NPU instead. One small,
suggestive detail: RGA's feature bitmask includes `RGA_NN_QUANTIZE`, a hint
that RGA's real intended role in Rockchip's own IPC/camera reference designs is
*feeding* the NPU (resize/convert/quantize a frame before inference), not the
NPU feeding graphics back to RGA. That's the direction Rockchip designed this
silicon to work in, and it lines up with everything else in this document: RGA
-> pixels, NPU -> tensors, and the two don't trade places.

---

## 5. Recommendation

| | Worth prototyping? | Why |
|---|---|---|
| **NPU for any part of 3D rendering** | **No** | Not "how the hardware works" past vertex-transform, and even that stage isn't a net win at this UI's vertex counts once job-submit overhead and INT8 quantization of matrices/vertices are accounted for. |
| **NPU for 2D image filters (blur/sharpen/edge)** | **No** | RKIVE is idle, purpose-built, and needs no model-compile step; strictly better fit if this is ever wanted at all. |
| **NPU for super-resolution / style transfer / segmentation effects** | **No** | No input source (no camera) or no product need; also RAM-marginal on this SKU regardless. |
| **NPU for 2D affine transforms (rotate/scale) as "graphics"** | **No** | RGA already does this in fixed-function hardware, cheaper, already proven in production. |
| **Porting `rknpu.ko` to 6.18** | **Conditionally yes, but scope it for inference, not graphics** | Same bounded, evidence-backed effort class as the RGA port; keeps the door open for the platform wiki's actual identified NPU opportunity (a small non-visual classifier: audio, RS-485/sensor anomaly detection, touch-gesture patterns). Do not justify or scope the port around a graphics capability; it doesn't unlock one. |
| **A first NPU spike, if one is wanted for team familiarity** | **Only the already-identified real use case** | A tiny non-visual model (e.g. an RS-485 anomaly classifier), not a graphics stunt. This is the same conclusion the platform wiki already reached independent of this research. |

### Falsifiers

Flagging explicitly, per the instruction to distinguish settled facts from
things needing verification:

- **TRM Part 2 (register-level RKNPU documentation) does not exist publicly.**
  Everything in section 2's rasterization/texture/per-pixel-shading "no" verdicts is
  reasoned from the RKNN operator taxonomy and general 4th-generation RKNPU
  architecture knowledge, not from a register-level ground truth, because that
  ground truth isn't published anywhere, including to Luckfox's own engineers
  per a direct forum admission. (`soc-rv1106.md:114`) If Rockchip ever publishes
  register-level detail, or if a full RKNN supported-operator list surfaces with
  a gather/sample-style op this research didn't find, revisit.
- **No on-hardware NPU job-submission latency number exists anywhere in the
  wiki or SDK for this board.** The "dispatch overhead beats any small win"
  argument in section 2 is architectural (ioctl + DMA + blocking IRQ wait, vs. a
  same-thread NEON call) and is high-confidence, but a real measured number
  would strengthen or could in principle narrow it. Not asserted as measured
  here.
- **Which SKU (G2 vs G3) our boards actually carry is unconfirmed**, which
  leaves the exact TOPS ceiling and RAM budget open; doesn't change any verdict
  in this document (nothing here turns on the TOPS number), but is worth closing
  out anyway during the driver port.
