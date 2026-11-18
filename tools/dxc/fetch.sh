#!/usr/bin/env bash
# Unpacks the DXC workflow's archive for this platform into ./dxc for the release's dxc/ folder. The
# archive comes from this repository's dxc-<tag>-<pin> pre-release, <pin> being the first 8 hex digits of
# the sha256 of tools/dxc/SHA256SUMS, and must match tools/dxc/SHA256SUMS; a
# mismatch or a missing hash stops the release. Without the pre-release the launcher ships without dxc
# and the job warns.
set -euo pipefail
archive=$1
here=$(cd "$(dirname "$0")" && pwd)
tag="dxc-$(cat "$here/version")-$(sha256sum "$here/SHA256SUMS" | cut -c1-8)"
dl=$(mktemp -d)
if ! gh release download "$tag" -R "${GITHUB_REPOSITORY:-Swofty-Developments/SwoftyLauncher}" -p "$archive" -D "$dl"; then
    echo "::warning::the $tag release has no $archive; this release ships without dxc"
    exit 0
fi
line=$(grep " $archive\$" "$here/SHA256SUMS" || true)
if [ -z "$line" ]; then
    echo "::error::tools/dxc/SHA256SUMS has no hash for $archive"
    exit 1
fi
(cd "$dl" && echo "$line" | sha256sum -c -)
tar xzf "$dl/$archive"
test -f dxc/VERSION
echo "dxc $(cat dxc/VERSION) unpacked from $tag"
