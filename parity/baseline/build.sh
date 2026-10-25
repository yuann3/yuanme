#!/usr/bin/env bash
# Builds the Parity baseline (#39): Astro's dist/ of eyuan.me at REV, built
# as eyuan.me's deploy workflow builds it (Node 22, `npm ci`, `npm run build`),
# checked against the live site, and packed with what its Pages load from
# other origins, so the harness never goes to the network (#23).
#
#   parity/baseline/build.sh REV OUT_DIR
#
# Writes OUT_DIR/parity-baseline-<short rev>.tar.zst and its .sha256. Needs
# Node 22, git, curl, GNU tar and zstd. The live site is built on
# ubuntu-latest, and sharp's encoders aren't byte-identical across
# platforms, so build on linux/amd64; on another machine, through Docker:
#
#   docker run --rm --platform linux/amd64 -v "$PWD/parity:/parity:ro" \
#     -v "$OUT_DIR:/out" node:22 bash -c \
#     'apt-get update -qq && apt-get install -qq zstd && /parity/baseline/build.sh REV /out'
#
# The tarball is deterministic for a given dist/ and the same remote
# responses, so a rebuild can be checked against parity/baseline.lock.
set -euo pipefail

rev=${1:?usage: build.sh REV OUT_DIR}
out=${2:?usage: build.sh REV OUT_DIR}
live=https://eyuan.me
# Google Fonts answers each browser with different CSS. Every engine the
# harness runs reads woff2 and unicode-range, so one modern Chrome's answer
# serves all three.
ua='Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36'

case "$(node --version)" in
v22.*) ;;
*) echo "build.sh: needs Node 22, as eyuan.me's deploy workflow uses; found $(node --version)" >&2; exit 1 ;;
esac

work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
git clone -q https://github.com/yuann3/eyuan.me "$work/src"
git -C "$work/src" checkout -q --detach "$rev"
full=$(git -C "$work/src" rev-parse HEAD)
short=${full:0:7}
(cd "$work/src" && npm ci --no-audit --no-fund && npm run build)

stage="$work/stage"
mkdir -p "$stage/remote"
cp -R "$work/src/dist" "$stage/site"
cd "$stage"

# The pages, as the URLs the live site serves them at.
pages=$(cd site && find . -name index.html | sed -e 's|^\.||' -e 's|index\.html$||' | sort)

# 1. Every hashed asset a live Page links to is the one this build links to.
# Cloudflare rewrites the live HTML (#13), but not its /_astro/ URLs.
astro_urls() { grep -o '/_astro/[^"'"'"' )>,]*' | sort -u; }
bad=0
for page in $pages; do
  ours=$(astro_urls < "site${page}index.html" || true)
  theirs=$(curl -fsSL "$live$page" | astro_urls || true)
  if [ "$ours" != "$theirs" ]; then
    echo "build.sh: $page links to other /_astro/ files than $live$page:" >&2
    diff <(echo "$ours") <(echo "$theirs") >&2 || true
    bad=1
  fi
done

# 2. Every other file is byte for byte the live one. HTML is left out
# because of Cloudflare's rewrites.
while IFS= read -r file; do
  path=${file#site}
  ours=$(sha256sum < "$file" | cut -d' ' -f1)
  theirs=$(curl -fsSL "$live$path" | sha256sum | cut -d' ' -f1)
  if [ "$ours" != "$theirs" ]; then
    echo "build.sh: $path differs from $live$path" >&2
    bad=1
  fi
done < <(find site -type f ! -name '*.html' | sort)
if [ "$bad" = 1 ]; then
  echo "build.sh: this build of $short isn't the live site" >&2
  exit 1
fi

# 3. What the Pages load from other origins, so the harness never goes to
# the network: the Google Fonts stylesheets and every font file they name,
# and the images Project pages embed from GitHub. remote/index.json maps
# each URL to its file and content type.
index='{}'
fetch() { # URL FILE
  mkdir -p "$(dirname "remote/$2")"
  type=$(curl -fsSL -A "$ua" -o "remote/$2" -w '%{content_type}' "$1")
  index=$(node -e 'const [i,u,f,t]=process.argv.slice(1);const o=JSON.parse(i);o[u]={file:f,type:t};console.log(JSON.stringify(o))' "$index" "$1" "$2" "$type")
}
for css in $(grep -rho 'https://fonts.googleapis.com/css2?[^"]*' site --include='*.html' | sed 's/&amp;/\&/g' | sort -u); do
  name="fonts.googleapis.com/$(printf %s "$css" | sha256sum | cut -c1-16).css"
  fetch "$css" "$name"
  for font in $(grep -o 'https://fonts.gstatic.com/[^)]*' "remote/$name" | sort -u); do
    fetch "$font" "${font#https://}"
  done
done
for img in $(grep -rhoE '<img [^>]*src="https://[^"]*"' site --include='*.html' | grep -oE 'https://[^"]*' | sort -u); do
  fetch "$img" "${img#https://}"
done
node -e 'const o=JSON.parse(process.argv[1]);const s={};for(const k of Object.keys(o).sort())s[k]=o[k];console.log(JSON.stringify(s,null,1))' "$index" > remote/index.json

cat > BASELINE.json <<EOF
{
 "repository": "https://github.com/yuann3/eyuan.me",
 "rev": "$full",
 "node": "$(node --version)",
 "npm": "$(npm --version)",
 "astro": "$(node -p 'require("'"$work"'/src/node_modules/astro/package.json").version')"
}
EOF

name="parity-baseline-$short"
mkdir -p "$out"
tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner --format=posix \
  --pax-option=exthdr.name=%d/PaxHeaders/%f,delete=atime,delete=ctime \
  -cf - BASELINE.json remote site | zstd -q -19 -T1 > "$out/$name.tar.zst"
(cd "$out" && sha256sum "$name.tar.zst" > "$name.tar.zst.sha256" && cat "$name.tar.zst.sha256")
