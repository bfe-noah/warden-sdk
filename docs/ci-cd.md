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
`bfe-mpc-0640-warden-sdk`, in `~/actions-runner-warden-sdk`. The registration is
done; the following steps need **`user`'s sudo on 0640** and are not automatable
from the dev box:

1. **Install as a service** (persistence): `cd ~/actions-runner-warden-sdk &&
   sudo ./svc.sh install user && sudo ./svc.sh start`. Until then the runner is
   *offline* and `kernel-build` only runs when dispatched against an online runner.
2. **Resource cap** (protect the shared host): a drop-in at
   `/etc/systemd/system/actions.runner.bfe-noah-warden-sdk.*.service.d/*.conf` with
   `CPUQuota=400%` + `MemoryMax=6G`, then `sudo systemctl daemon-reload`. The build
   inherits that cgroup. (The workflow also passes `JOBS=4` as a belt-and-braces bound.)
3. **Kernel cross toolchain**: `build-kernel.sh` needs the arm cross compiler on
   PATH — set `SDK_TC` to the dir holding `arm-rockchip830-linux-uclibcgnueabihf-*`
   (the Luckfox SDK toolchain, as flare-edge's runner has), or install
   `gcc-arm-linux-gnueabihf` and pass `CROSS_COMPILE=arm-linux-gnueabihf-` (the
   kernel is freestanding, so a generic arm cross compiler links it).
4. **`python`** (not python3): the kernel build calls bare `python`; provide a
   project-local venv or a `python`→`python3` shim on the runner's PATH.

Host build deps (sudo): `dtc bc flex bison libssl-dev` (already present on 0640).

## Badges

Static shields SVGs are committed by the `badges` job (private repo can't use
dynamic shields). The GitHub-native `ci.yml` status badge works live regardless.
