# warden-sdk — Code Review Report (code-review-v0.0.2)

**Scope:** our authored SDK code on branch `bringup` — `sim/` (Rust hardware
simulator), `tools/config-lint` + `tools/flowgen.py`, `drivers/{relays,freshness}`
(C + MC/DC harnesses), `build/build-kernel.sh` + `build/warden_defconfig`, and
`.github/workflows/ci.yml`. **Excluded:** `patches/` (223-file / ~136K-line vendored
+ ported RV1106 kernel delta — not our code to review) and `kernel/rv1106-enablement`
provenance docs.

**Gate:** pre-first-commit-to-`main` review for the SDK-completion work.

## Verdict

✅ **READY TO SHIP** (to `main`, subject to Noah's go-ahead).

No Critical or High finding remains open. Every finding at every severity — the
workspace rule is "correct ALL flagged items at ALL severity levels" — was fixed and
verified. The recursive loop converged after **2 iterations**; iteration 2 was an
adversarial re-review that fuzz/empirically cleared the bulk of the iteration-1 fixes
and surfaced only one fix-of-a-fix plus doc-accuracy corrections.

## Codebase summary (authored, review scope)

| Metric | Value |
|---|---|
| Commits on `bringup` | 53 |
| Rust (sim + config-lint, incl. tests) | 1,918 lines |
| C drivers (relays, freshness) | 471 lines + 424 lines of MC/DC harness |
| Shell (build + shared gate) | 253 lines |
| Python (flowgen) | 169 lines |
| Rust tests | 46 (sim 37, config-lint 9) |
| C MC/DC conditions | **106 / 106 = 100%** (relays 40/40, freshness 66/66) |
| ADRs | 5 (`docs/decisions/0001`–`0005`) |
| Kernel forward-port (excluded) | 13 subsystem patches, 223 file-deltas, ~136K lines |

## Iteration history

| Iter | Reviewers | Findings | Changes applied | CI |
|---|---|---|---|---|
| 1 | 4 dimensions (security, correctness/reliability, maintainability/docs, test-quality/perf) | 1 Critical, 4 High, 7 Medium, 6 Low | 18 fixes across 15 files (commit `6856447`) | ✅ green |
| 2 | 2 adversarial re-reviewers (correctness re-verify, doc-accuracy re-verify) | 1 High (fix-of-a-fix), 2 Medium (pre-existing doc inaccuracies), 1 Minor (ADR staleness) | 4 fixes (commit `7044585`) | ✅ green |

**Convergence:** iteration 2 fuzz/empirically verified the iteration-1 fixes were
correct (freshness min-budget: 200k random trials all matched a ground-truth min;
config-lint reg-scanner + UTF-8 safety: no misses/panics; build-kernel trap: 5 exit
scenarios all correct). The only new *code* issue was a fix-of-a-fix (loader
substring), now closed with the same whole-name rigor and a regression test using the
reviewer's adversarial names. Remaining items were factual doc corrections. No third
iteration was warranted (no new actionable findings expected).

## Blocking findings (Critical + High) — all fixed

| # | Sev | Location | Finding | Fix |
|---|---|---|---|---|
| 1 | Critical | `docs/architecture.md` §6 | Described the superseded plan44/OpenWrt 6.6 kernel plan as the future direction — ADR-0001 supersedes it with the direct 5.10→6.18.46 forward-port (done, hardware-verified). | Rewrote §6 to match ADR-0001. |
| 2 | High | `drivers/freshness/freshness.c:181` | `warden_fresh_min_budget_ms()` used `best == 0` as the empty sentinel, so a legitimate zero-tolerance binding (`max_stale_ms == 0`) was silently widened to a looser neighbour's budget — a real freshness-contract violation. | Separate `bool seen` flag; regression test; still 66/66 MC/DC. Fuzz-verified (200k trials). |
| 3 | High | `build/build-kernel.sh:40` | sha256 verification was fail-open (silently skipped if the `.sha256` pin was missing) — a KVER bump or a `KERNEL_TARBALL` pointed anywhere would build from an unverified tarball. | Fail closed: refuse to build if the pin is absent. |
| 4 | High | `README.md` status blurb | "bootstrapping… nothing on the production build path" contradicted the done kernel/patches/drivers work two paragraphs down. | Rewrote to "bringup" with the accurate remaining-work list. |
| 5 | High | `docs/architecture.md` §3 §7 | NPU/RGA models, config-lint, and the relays/freshness MC/DC harnesses were listed as not-yet-built / pending when they exist. | Moved to Done; §7 order-of-work marked accurately. |
| 6 | High | `tools/config-lint/src/lib.rs` (iter 2) | `is_known_safe_loader` (iteration-1's fail-closed allowlist) matched tokens as unanchored substrings, so `AudioLoader`/`SplRtos`/`Bl32` would be waved through — reopening the 0x40000-brick false-negative. | Whole normalized-name match; regression test with those exact names. |

## Non-blocking findings (Medium / Low) — all fixed

- **[M]** `config-lint parse_reserved_ranges` split on the bare substring `"reg"`, so a `region-*` label / `reg-names` misparsed into a bogus reserved range → whole-token match (boundary before, `=` after). Regression test added.
- **[M]** `config-lint is_mcu_loader` was a denylist (only known MCU names) → replaced with a fail-closed allowlist (later hardened to whole-name in iter 2).
- **[M]** `build-kernel.sh` leaked a >1GB `mktemp` scratch tree on the shared SDK box → `trap … EXIT`, cleaning up only a script-owned `WORK` (caller-provided CI `WORK` is preserved for artifact upload). Verified across 5 exit scenarios.
- **[M]** `architecture.md` §4 Tier-1 list omitted `freshness.c` → added.
- **[M]** `freshness.{c,h}` cited "ADR 0004" — collides with warden-sdk's own ADR-0004 (CI runner) → points at flare-edge ADR-0004 (the real Freshness Contract ADR).
- **[M]** `README` layout listed empty `ci/` → corrected to `.github/workflows/ci.yml`; stale `patches/`/`sim/` descriptions fixed.
- **[M]** `architecture.md` §3 `cru` bullet overclaimed the flared devmem `Bus` seam as shipped (it's on an unmerged flare-edge branch) → scoped to pending, matching §7.
- **[M]** `drivers/README.md` said modbus "11 pty scenarios" → "8 pty scenarios + 3 wire/daemon checks (11 total)".
- **[L]** `ci.yml`: no explicit least-privilege permissions + `taiki-e/install-action@v2` mutable tag → top-level `permissions: contents: read` (badges overrides to write) + pinned to commit SHA (`v2.86.7`).
- **[L]** `README` layout omitted `kernel/` → added.
- **[L]** Two near-identical `enforce-mcdc.sh` copies → one shared `drivers/enforce-mcdc.sh` deriving the driver name from the `.gcov` arg; both Makefiles call it.
- **[L]** `freshness` `age == max_stale` boundary and clock-wraparound (`now < last_ok`, ~49.7-day uint32 wrap → fail-safe to UNKNOWN) were untested → both tests added.
- **[L]** `relays` test relied on ambient env for the `getenv`-NULL arm → `unsetenv(WARDEN_GPIO_ROOT)` at `main()` for hermeticity.
- **[L]** hpmcu "8 tests" → 7 in `architecture.md` and `drivers/README.md`; ADR-0002 Consequences updated to the shared-gate layout + Rust MC/DC tooling reality.

## Verified sound (not defects — checked, since they were the explicit worries)

- MC/DC gate is not gameable: removing a single test case drops coverage below 100% and fails CI (empirically confirmed by a reviewer).
- Modbus RTU slave: thorough length/range validation before every PDU index; `snprintf` bounds in relays correct.
- config-lint parsers: no UTF-8 byte-boundary panic (ASCII anchors only); `parse_reserved_ranges` is linear.
- CI jobs genuinely gate (GitHub's default `bash -e {0}`; the `test`-job loop aborts on the first failing crate — reproduced).
- No hardcoded secrets (gitleaks clean); CI token scoping correct; benchmarks time real dispatch paths.

## Test + benchmark status

- **Tests:** sim 37, config-lint 9, relays 31 checks, freshness 32 checks — all green.
- **MC/DC:** 106/106 conditions (relays 40, freshness 66) at 100%, CI-enforced via the shared `drivers/enforce-mcdc.sh` (`mcdc` job).
- **Benchmarks:** `sim/benches/sim_bench.rs` times 5 modelled blocks (hpmcu_tick, cru_poll, modbus_read_holding, rga_improcess, membus_poke_peek); ns/op + JSON trend emitted by the `bench` job. Regression gating against stored history is documented future work.
- **Patch series:** applies cleanly onto pristine `linux-6.18.46` (`patches-apply` job, green).

## Tooling gaps (noted, not blocking)

- **Rust MC/DC:** deferred on tooling skew — nightly `-Z coverage-options=condition` needed; Rust uses `cargo-llvm-cov` line/region coverage, the C drivers carry the literal MC/DC gate (ADR-0002).
- **pylint** on Python 3.13 broke (`ImportError: formatargspec`); `flowgen.py` validated via `py_compile` instead.
- **config-lint is not yet wired as a CI *gate* against a real target `.ini`+DT** — only its unit tests run in CI. Wiring the CLI against the built board DT is a follow-up (needs the target artifacts in-repo).

## Human review checklist (advisory — not part of the verdict)

- Confirm the `is_known_safe_loader` allowlist covers every RK boot-component name your real rkbin `.ini`s use (over-inclusion only over-reports; a missed coprocessor is the dangerous direction).
- P5 remainder (Noah-gated): install the `[self-hosted, warden-sdk]` runner service on bfe-mpc-0640 + cgroup cap + `python`/toolchain provisioning (documented in `docs/ci-cd.md`); until then `kernel-build` stays `workflow_dispatch`-gated (and correctly skips).
- P7: first commit to `main` needs your fresh explicit go-ahead (per workspace rules).

## Automated actions taken

- 22 findings fixed across 2 iterations; 2 commits (`6856447`, `7044585`), both CI-green.
- 4 new regression tests (freshness zero-tolerance, boundary, wraparound; config-lint reg-token, unknown-coprocessor, whole-name-loader).
- `cargo fmt` normalized pre-existing rustfmt drift; two `enforce-mcdc.sh` copies de-duplicated.
- gitleaks clean on both commits; `.claude/` + `__pycache__/` gitignored (kept out of the OSS distribution).
