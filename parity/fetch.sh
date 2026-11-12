#!/usr/bin/env bash
# Downloads the Parity baseline that baseline.lock pins, checks its sha256
# and unpacks it into DIR: DIR/site, DIR/remote and DIR/BASELINE.json.
#
#   parity/fetch.sh DIR
#
# PARITY_BASELINE=FILE unpacks a local tarball instead, such as one
# baseline/build.sh wrote. Exits 3 if the release isn't published yet.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
dir=${1:?usage: fetch.sh DIR}
field() { sed -n "s/^$1 = \"\(.*\)\"$/\1/p" "$here/baseline.lock"; }
tag=$(field tag)
sha=$(field sha256)
mkdir -p "$dir"

tarball=${PARITY_BASELINE:-$dir/$tag.tar.zst}
if [ -z "${PARITY_BASELINE:-}" ] && [ ! -f "$tarball" ]; then
  # Only a release that isn't there skips the test: a missing gh or a
  # failed request would otherwise pass Parity without comparing anything.
  if ! err=$(gh release view "$tag" -R yuann3/yuanme 2>&1 > /dev/null); then
    if [ "$err" = "release not found" ]; then
      echo "fetch.sh: the release $tag isn't published" >&2
      exit 3
    fi
    echo "fetch.sh: can't look up the release $tag: ${err:-gh failed}" >&2
    exit 1
  fi
  gh release download "$tag" -R yuann3/yuanme -p "$tag.tar.zst" -D "$dir"
fi
actual=$(sha256sum < "$tarball" | cut -d' ' -f1)
if [ "$actual" != "$sha" ]; then
  echo "fetch.sh: $tarball has sha256 $actual; baseline.lock pins $sha" >&2
  exit 1
fi
rm -rf "$dir/site" "$dir/remote"
zstd -dc "$tarball" | tar -xf - -C "$dir"
echo "fetch.sh: $tag in $dir"
