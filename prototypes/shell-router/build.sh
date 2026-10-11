#!/usr/bin/env bash
# Build the three dists for prototype #26 with Bun.
#   out/astro-dist       stock eyuan.me (Astro <ClientRouter />)
#   out/proto-dist       eyuan.me + shell-router.patch (Navigation API router)
#   out/proto-hist-dist  the same + the History API driver
# Usage: ./build.sh            (clones eyuan.me at the pinned commit into ./eyuan.me if missing)
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
site="$here/eyuan.me"
out="$here/out"
rev=6567e01
mkdir -p "$out"

if [ ! -d "$site/.git" ]; then
  git clone -q https://github.com/yuann3/eyuan.me "$site"
  git -C "$site" remote set-url --push origin no-push
fi
git -C "$site" checkout -q "$rev" 2>/dev/null || git -C "$site" checkout -q --detach "$rev"
git -C "$site" reset -q --hard "$rev"
(cd "$site" && bun install >/dev/null)

build() { (cd "$site" && bun --bun run astro build >/dev/null) && rm -rf "$out/$1" && cp -R "$site/dist" "$out/$1"; echo "built $1"; }

build astro-dist
git -C "$site" apply "$here/shell-router.patch"
build proto-dist
perl -pi -e "s#^(import \{ startFilm \} from './film';)#import './router-history';\n\$1#" "$site/src/scripts/shell.ts"
build proto-hist-dist
git -C "$site" reset -q --hard "$rev"
git -C "$site" clean -qfd src
