# CI/CD

`.github/workflows/ci.yml`: every job runs on GitHub-hosted `ubuntu-latest`.
No self-hosted runner is (or may be) reachable from this repo's workflows:
on a public repo, a fork PR that gets one approved run could otherwise
execute code on private infrastructure (ADR-0007).

## Jobs

| Job | Runner | What it does |
|---|---|---|
| `test` | ubuntu-latest | `cargo test` (sim + config-lint + qemu/rs485-bridge) + `cargo-llvm-cov` line coverage on `sim`; outputs `passed`/`coverage`. |
| `mcdc` | ubuntu-latest | 100% MC/DC enforced on every `drivers/*/test` (gcc-14 `-fcondition-coverage`). |
| `bench` | ubuntu-latest | Smoke-runs the sim + rs485-bridge micro-benchmarks; emits ns/op trend JSON. |
| `patches-apply` | ubuntu-latest | Fetches pristine linux-6.18.46 (cached, sha256-verified) and applies `patches/*` in order. |
| `qemu-tools` | ubuntu-latest | shellcheck on `qemu/**.sh`; builds the initramfs (pinned busybox) and the A/B disk image. |
| `quality` | ubuntu-latest | Codacy-style grade computed in-pipeline: clippy, cppcheck, shellcheck, ruff, lizard, jscpd, cargo-audit feed `tools/quality/score.py` (SQALE debt ratio + a separate worst-of security axis; SonarQube's published thresholds). Uploads `quality.json`; fails if the security grade is worse than C. |
| `kernel-build` | ubuntu-latest, **dispatch-only** | apt-installs the cross toolchain + qemu, `build/build-kernel.sh` -> `zImage` + `rv1106-warden.dtb`, QEMU `-M virt` boot smoke (fail-closed), artifact upload (best-effort). Trigger: `gh workflow run ci.yml`. |
| `prune-artifacts` | ubuntu-latest, dispatch-only | Deletes `kernel-rv1106` artifacts beyond the newest 3. |
| `badges` | ubuntu-latest | Renders loc/tests/coverage shields on push to `main` (`[skip ci]` + `paths-ignore` loop guard). |

## Runner History

`kernel-build` originally ran on a repo-scoped self-hosted runner (ADR-0004)
because hosted minutes were metered on the private repo. Going public made
hosted minutes free and a self-hosted registration a fork-PR liability, so
ADR-0007 retired it. That runner's site records live in the private
deployment log, not here.

## Badges

SVGs are rendered in-runner with anybadge and committed by the `badges`
job: no external badge or assessment service at render or view time. The
quality letter comes from the `quality` job; `tools/quality/score.py`
documents the scoring model and thresholds. The GitHub-native `ci.yml`
status badge works live regardless.
