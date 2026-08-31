# RGA port: DONE, verified on hardware (2026-08-24)

Ported the vendor char-dev RGA driver (`drivers/video/rockchip/rga3/`, the
multicore `CONFIG_ROCKCHIP_MULTI_RGA` driver: WardenOS's librga uses `/dev/rga`,
not V4L2) to 6.18. Open source (vendor C source). On warden-c8a3:
```
rga: rga2, irq = 55, match scheduler
rga: rga2 hardware loaded successfully, hw_version:3.3.87975
rga: rga2 probe successfully        ->  /dev/rga present, /dev/dma_heap present
```

## Build tree wiring
`drivers/video/rockchip/{Kconfig,Makefile}` created (source rga3/); wired into
`drivers/video/{Kconfig,Makefile}`. `CONFIG_ROCKCHIP_MULTI_RGA=y`.
`&rga2 { status="okay"; }`. For `/dev/dma_heap/cma`: use the kernel's default 64 MiB
CMA + `CONFIG_DMABUF_HEAPS{,_CMA,_SYSTEM}=y`; do NOT add a `linux,cma` DT node
(a mis-aligned/duplicate one hangs the boot at reserved-memory init).

## 5.10 -> 6.18 API deltas fixed (all mechanical)
1. `platform_driver.remove`: `int` -> `void` (`rga_drv_remove`, drop the returns).
2. `hrtimer_init(&t,...)` + `t.function=fn` -> `hrtimer_setup(&t, fn, CLOCK_MONOTONIC,
   HRTIMER_MODE_REL)`: the scheduler tick; verified under the live probe.
3. `iommu_map` / `iommu_map_sg`: added the new `gfp` arg (`GFP_KERNEL`).
4. `get_user_pages_remote`: dropped the removed `vmas` arg.
5. `MAX_ORDER` (removed) -> compat `#define MAX_ORDER (MAX_PAGE_ORDER + 1)` in
   `rga3/include/rga_drv.h` (preserves the old exclusive semantics everywhere).
6. `MODULE_IMPORT_NS(<token>)` -> `MODULE_IMPORT_NS("DMA_BUF")` (string form).
