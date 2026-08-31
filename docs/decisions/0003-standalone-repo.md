# ADR 0003 — Standalone Repo

**Status:** Accepted (2026-08-25). Repo-visibility half superseded by ADR-0007
(2026-08-30) — warden-sdk went public; the "private for now" consequence below
no longer holds. Original decision kept for the record.

## Context
Our real SDK changes lived as uncommitted edits in a 2GB opaque vendor fork, with
no CI, tests, or versioning of their own. The SDK requirement (future-features-2
§SDK) calls for "its own repo, held to firmware standards."

## Decision
A **private** `warden-sdk` GitHub repo (now `blueflare-energy/bfe-core1106-sdk` and public
per ADR-0007), standalone from day one with its own
CI/versioning. Work lands on a `bringup` branch; the first commit to `main` is gated
on a passing review run, green CI, and the maintainer's fresh explicit go-ahead.

## Consequences
- flare-edge consumes this repo later (flared depending on `warden-sim`, drivers
  built from here) — a separate, maintainer-gated integration step; flare-edge is not
  edited by the SDK-completion effort.
- Private for now (references bench devices / in-progress hardening); can be opened
  later once scrubbed, matching how `flare-deployment` is handled.
