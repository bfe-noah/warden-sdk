# NPU (rknpu) open kernel driver: VERIFIED on warden-c8a3 (2026-08-25)

The open GPL rknpu kernel driver runs on our self-built Linux 6.18.46. This is the
achievable open end state (Tier A in `OPEN-NPU-PLAN.md`); open *compute* remains a
from-scratch RE project (see the ceiling note below).

## Evidence (serial, _b slot = our 6.18)
```
[drm] Initialized rknpu 0.9.2 for ff660000.npu on minor 1
/dev/dri/  -> card0 (VOP display)  card1 (rknpu)  renderD128
$ rknpu_version_test
  /dev/dri/card0: not an rknpu node (version ioctl: Invalid argument)
  opened /dev/dri/card1 (fd=3)
  driver version: 0.9.2 (raw code 902)
  hw version: 0x54524548 (raw)
  PASS: /dev/dri/card1 answered both version-query ioctls -- probe,
        power-get/put, and clock/reset all exercised.
```
The `RKNPU_GET_DRV_VERSION`/`RKNPU_GET_HW_VERSION` ioctls exercise the full
dispatch -> power-domain get/put -> clock/reset path, so a clean answer confirms the
whole driver bring-up, not just registration.

## The fix that took it from probe-fail to PASS
The base dtsi `npu@ff660000` node declares its interrupt but has **no
`interrupt-names`**; the rknpu driver requests its IRQ by name (`"npu_irq"`), so
probe bailed `error -ENXIO: IRQ npu_irq not found` and never registered its DRM
device. Board DTS override adds `interrupt-names = "npu_irq";` (+ `status="okay"`).
(The rest of the port (GPL source, 4 compat-shim headers for dead-code vendor
headers, 10 mechanical 6.18 API deltas) is in `PORT-PROGRESS.md`.)

Also: the version test must iterate DRM cards and keep the one that ANSWERS the
ioctl. The display card (card0) opens fine but returns EINVAL. Fixed in
`rknpu_version_test.c`.

## The honest ceiling (why "100% open NPU" stops at the driver)
The kernel driver only DMAs an opaque userspace-authored `regcmd` blob into the PC
registers and pulses go. It never inspects the compute stream. The compute-engine
register map (TRM Part 2) is not public for RV1106, there is **zero open RE prior
art** for this NPU generation, and only the closed RKNN-Toolkit2 compiler emits
valid regcmd. Mainline `accel/rocket` + Mesa Teflon are RK3588-only (64-bit). So an
open compiler/runtime is a from-scratch, ~person-year register-RE project, scoped
(not staffed) in `OPEN-NPU-PLAN.md` (M-NPU-2 spike / M-NPU-3). We ship the open
driver; we do NOT ship any closed blob.
