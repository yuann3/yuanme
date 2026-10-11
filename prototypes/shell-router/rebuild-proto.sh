#!/usr/bin/env bash
# Iteration helper: rebuild proto + proto-hist from the working branch in ./eyuan.me
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; site="$here/eyuan.me"
cd "$site"
git add -A src && git commit -qm "wip" || true
bun --bun run astro build >/dev/null 2>&1 && rm -rf "$here/out/proto-dist" && cp -R dist "$here/out/proto-dist"
perl -pi -e "s#^(import \{ startFilm \} from './film';)#import './router-history';\n\$1#" src/scripts/shell.ts
bun --bun run astro build >/dev/null 2>&1 && rm -rf "$here/out/proto-hist-dist" && cp -R dist "$here/out/proto-hist-dist"
git checkout -q src/scripts/shell.ts
ls -la "$here"/out/proto*/_astro/Shell*.js | awk '{print $5, $9}'
