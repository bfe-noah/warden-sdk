#!/usr/bin/env bash
# Fetch (with retries) and sha256-verify the pristine kernel tarball into $1.
# Single source of truth for the URL + verification used by build-kernel.sh
# and both CI jobs (patches-apply, kernel-build) — a KVER bump edits this file
# and build-kernel.sh only. FAILS CLOSED: a missing pin refuses to proceed.
#
# Usage: fetch-kernel-tarball.sh <destination-path>
set -euo pipefail

KVER=6.18.46
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"     # build/
SHA_FILE="$HERE/linux-$KVER.tar.xz.sha256"
URL="https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-$KVER.tar.xz"

TB="${1:?usage: fetch-kernel-tarball.sh <destination-path>}"

if [ ! -f "$TB" ]; then
  echo "== downloading $URL"
  curl --retry 3 --retry-delay 5 -fSL "$URL" -o "$TB"
fi
[ -f "$SHA_FILE" ] || {
  echo "FATAL: no pinned sha256 for linux-$KVER (expected $SHA_FILE) — refusing an unverified tarball" >&2
  exit 1
}
want="$(cat "$SHA_FILE")"
got="$(sha256sum "$TB" | awk '{print $1}')"
[ "$want" = "$got" ] || { echo "FATAL: tarball sha256 mismatch: want $want got $got" >&2; exit 1; }
echo "== tarball sha256 verified: $TB"
