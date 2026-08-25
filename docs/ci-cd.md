# CI/CD

`.github/workflows/ci.yml` — everything portable runs on GitHub-hosted
`ubuntu-latest`; only the heavy kernel build uses the self-hosted runner.

## Jobs

| Job | Runner | What it does |
|---|---|---|
| `test` | ubuntu-latest | `cargo test` (sim + config-lint) + `cargo-llvm-cov` line coverage on `sim`; outputs `passed`/`coverage`. |
| `mcdc` | ubuntu-latest | 100% MC/DC enforced on every `drivers/*/test` (gcc-14 `-fcondition-coverage`). |
| `bench` | ubuntu-latest | Smoke-runs the sim micro-benchmarks; emits ns/op trend JSON. |
| `patches-apply` | ubuntu-latest | Fetches pristine linux-6.18.46 (cached, sha256-verified) and applies `patches/*` in order. |
| `kernel-build` | **[self-hosted, warden-sdk]** | `build/build-kernel.sh` → `zImage` + `rv1106-warden.dtb`, uploaded as an artifact. Dispatch-gated until the runner is fully provisioned (below). |
| `badges` | ubuntu-latest | Renders loc/tests/coverage shields on push to `main` (`[skip ci]` + `paths-ignore` loop guard). |

## The self-hosted runner (`bfe-mpc-0640`)

A **third** repo-scoped runner instance on `bfe-mpc-0640` (alongside `flare` and
`flare-edge`), registered with the label **`warden-sdk`** as
`bfe-mpc-0640-warden-sdk`, in `~/actions-runner-warden-sdk`.

> **INSTALLED + ONLINE (2026-08-25).** The runner is a running systemd service
> (`actions.runner.bfe-noah-warden-sdk.bfe-mpc-0640-warden-sdk.service`, `enabled`,
> cgroup-capped `CPUQuota=400%`/`MemoryMax=6G`) and `kernel-build` has been verified
> end-to-end (RV1106 6.18.46 → `zImage` 8.25 MB + `rv1106-warden.dtb`). Steps 1–2
> below are the record of that install (they needed `user`'s sudo on 0640); steps
> 3–4 are handled inside the workflow, so the host needs no manual toolchain/python.

1. **Install as a service** (persistence): `cd ~/actions-runner-warden-sdk &&
   sudo ./svc.sh install user && sudo ./svc.sh start`. Until then the runner is
   *offline* and `kernel-build` only runs when dispatched against an online runner.
2. **Resource cap** (protect the shared host): a drop-in at
   `/etc/systemd/system/actions.runner.bfe-noah-warden-sdk.*.service.d/*.conf` with
   `CPUQuota=400%` + `MemoryMax=6G`, then `sudo systemctl daemon-reload`. The build
   inherits that cgroup. (The workflow also passes `JOBS=4` as a belt-and-braces bound.)
3. **Kernel cross toolchain** — done in the workflow: the `kernel-build` job sets
   `CROSS_COMPILE=arm-linux-gnueabihf-` (Debian `gcc-arm-linux-gnueabihf`, already on
   the runner) and `build-kernel.sh` honors it. The kernel is freestanding, so the
   generic arm cross compiler links it — no Luckfox SDK toolchain path needed. (To use
   the SDK uclibc toolchain instead, set `SDK_TC` to its `bin/` and drop the override.)
4. **`python`** (not python3) — done in the workflow: the `kernel-build` job symlinks
   `python`→`python3` into `$RUNNER_TEMP/bin` and prepends it to `$GITHUB_PATH`. No
   host-side venv/shim needed.

Host build deps: `dtc bc flex bison libssl-dev` — already present on 0640.

## Badges

Static shields SVGs are committed by the `badges` job (private repo can't use
dynamic shields). The GitHub-native `ci.yml` status badge works live regardless.
