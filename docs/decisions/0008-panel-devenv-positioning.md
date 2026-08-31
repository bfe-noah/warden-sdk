# ADR 0008 — Panel Development Environment

**Status:** Accepted (2026-08-30).

## Context
warden-sdk was written as the SDK for WardenOS, and its documentation framed
it that way: a support repo for one product. Since going public (ADR-0007,
GPL-2.0-only), the actual audience is wider — anyone with a Luckfox Pico 86
Panel gets a maintained 6.18 kernel, an off-device development loop, and a
device simulator out of this repo, none of which exists elsewhere for this
board. The product-first framing undersold that and confused the entry point
for outside readers.

## Decision
Position warden-sdk as **a modern, open development environment for the
Luckfox Pico 86 Panel (RV1106)**. WardenOS is documented as the downstream
consumer it is, not the purpose. Documentation follows three rules: lead with
the board, not the product; keep private-repo references clearly marked as
context; keep titles short — a heading names a section, it does not summarize
it.

## Consequences
- README and top-level docs lead with the hardware and the developer loop
  (build, simulate, test), with verified quick-start commands.
- WardenOS/flare-edge specifics stay where they are engineering truth (the
  seam inventory, scenario payloads) but read as one consumer's usage.
- The honesty rule is unchanged: emulation results are never on-silicon
  claims.
- The repo is renamed **`bfe-core1106-sdk`** (2026-08-31): the name leads
  with the org and the chip, not the product. `warden-sdk` remains as a
  GitHub redirect; crate names (`warden-sim`, `warden-config-lint`), the
  `WARDEN_*` env vars, and binary names are unchanged.
