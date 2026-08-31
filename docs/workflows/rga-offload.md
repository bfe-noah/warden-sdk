# RGA Offload

> **Outcome tested:** Blits go to the RGA when it succeeds, and fall back to the CPU when it doesn't.

**Benchmark** (`rga_improcess`): 6.7 ns/op

```mermaid
flowchart TD
  A[draw request] --> B{RGA compiled in\n(#if WARDEN_USE_RGA)?}
  B -- no --> C[LVGL software draw]
  B -- yes --> D[improcess src,dst,rects IM_SYNC]
  D --> E{IM_STATUS == SUCCESS?}
  E -- yes --> F[done on RGA]
  E -- no --> C
```
