# ADR 0003 — warden-sdk is a standalone repo

**Status:** Accepted (2026-08-25).

## Context
Our real SDK changes lived as uncommitted edits in a 2GB opaque vendor fork, with
no CI, tests, or versioning of their own. The SDK requirement (future-features-2
§SDK) calls for "its own repo, held to firmware standards."

## Decision
A **private** `bfe-noah/warden-sdk` GitHub repo, standalone from day one with its own
CI/versioning. Work lands on a `bringup` branch; the first commit to `main` is gated
on a passing code-review-harness run, green CI, and Noah's fresh explicit go-ahead.

## Consequences
- flare-edge consumes warden-sdk later (flared depending on `warden-sim`, drivers
  built from here) — a separate, Noah-gated integration step; flare-edge is not
  edited by the SDK-completion effort.
- Private for now (references bench devices / in-progress hardening); can be opened
  later once scrubbed, matching how `flare-deployment` is handled.
