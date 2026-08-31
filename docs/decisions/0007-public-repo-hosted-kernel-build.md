# ADR 0007: Hosted-Only CI

**Status:** Accepted (2026-08-30). Supersedes the runner half of ADR-0004.

## Context
The repo is going public (free Actions minutes for hosted runners; open-source
alignment with the stack philosophy). Two facts change the ADR-0004 calculus:

1. **A self-hosted runner on a public repo is a standing hazard.** A fork PR
   can modify workflow files; once any run of theirs is approved, workflows
   can target the repo's registered self-hosted runners, i.e. arbitrary code
   on the private host, which also serves production. GitHub's own guidance is
   to never attach self-hosted runners to public repos, and personal-account
   repos have no runner groups to scope the risk away.
2. **The build never needed the SDK host.** ADR-0004's premise ("needs the SDK
   toolchain and Buildroot's baked-in absolute paths") does not apply to
   `kernel-build`: the hermetic build is freestanding, uses Debian's
   `gcc-arm-linux-gnueabihf`, and self-provisions `python`. It fits a hosted
   runner (4 vCPU / 16 GB), and public-repo minutes are free.

## Decision
`kernel-build` runs on `ubuntu-latest`, apt-installing its toolchain, kernel
build deps, and qemu-system-arm, with the pristine tarball cached like
`patches-apply` does. It stays `workflow_dispatch`-only for now (a full build
per push is still noisy; flipping it to push-on-main later is one line). The
`warden-sdk` self-hosted runner instance is **deregistered from this repo**
before it goes public; the flare and flare-edge runner instances on the same
host are unaffected (those repos stay private).

## Consequences
- No path from public workflows to private infrastructure; nothing to babysit
  in fork-PR approval settings beyond GitHub's defaults (still set "require
  approval for all outside contributors" as belt-and-braces).
- Kernel artifacts no longer persist on the runner host; the GitHub artifact
  (5-day retention + prune job) is the only build output channel.
- Hosted kernel builds are slower than the 0640 box but free and parallel;
  the boot-smoke step rides along unchanged.
