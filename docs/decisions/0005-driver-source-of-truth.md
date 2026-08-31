# ADR 0005 — Driver Source of Truth

**Status:** Accepted (2026-08-25).

## Context
Our own hardware-facing driver code (relays, modbus master, RGA wrapper, HPMCU
supervisor, devmem/reset ladder, freshness) is the Tier-1 MC/DC target, but it
currently lives in flare-edge. The kernel driver source lives in an un-versioned
scratch tree (`flare-edge/research/linux-6.18.46/`).

## Decision
Bring **hardened copies into `warden-sdk/drivers/`** as the canonical source-of-truth,
each with its HAL seam and a 100% MC/DC host harness. The RV1106 kernel deltas are
formalized as a patch series in `patches/`. flare-edge consumes warden-sdk later
(separate, maintainer-gated step).

## Consequences
- Realizes the seam architecture (ADR-referenced in `docs/architecture.md`).
- flare-edge is not edited now; a later integration step points flare-edge at these.
- Risk: temporary duplication of shared constants between the two repos until the
  integration lands — tracked, acceptable for the bring-up window.
