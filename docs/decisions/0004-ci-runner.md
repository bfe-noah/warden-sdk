# ADR 0004: Self-Hosted Runner

**Status:** Superseded in part by ADR-0007 (2026-08-30): `kernel-build` moved
to GitHub-hosted runners for the public repo and the self-hosted registration
is retired. Original decision below, kept for the record. (2026-08-25.)

## Context
The heavy kernel/firmware build needs the SDK toolchain and Buildroot's baked-in
absolute paths, impractical on GitHub-hosted runners. flare-edge already builds on
a repo-scoped self-hosted runner on `bfe-mpc-0640` (label `flare-edge`); a repo-scoped
registration cannot be shared across repos by label alone.

## Decision
Register a **third repo-scoped runner instance** on `bfe-mpc-0640`, label
`warden-sdk` (its own repo-scoped runner systemd unit, own
`CPUQuota=400%`/`MemoryMax=6G` drop-in). Only the heavy `kernel-build` job uses
`runs-on: [self-hosted, warden-sdk]`; all host-testable jobs (tests, coverage, MC/DC,
benchmarks, badges, patches-apply) run on GitHub-hosted runners.

## Consequences
- Isolated from the flare backend + flare-edge CI already on that host (cgroup-capped).
- Runner setup + host build deps + the project-local `python` venv documented in
  `docs/ci-cd.md`, mirroring flare-edge's.
