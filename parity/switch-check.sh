#!/usr/bin/env bash
# The cutover's HTTP checks (#39, #80) on the deployed Reference site: the
# workers.dev preview before the switch, and https://eyuan.me right after
# it and after a rollback. Every Page of the Parity baseline and every URL
# in ORIGIN's sitemap is a 200, the feed keeps the baseline's guids, and
# the headers, the 404 and robots.txt are the cloudflare Host adapter's.
# With WWW, that origin must 301 to ORIGIN, keeping the path and query.
#
#   parity/switch-check.sh BASELINE ORIGIN [WWW]
#   parity/switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me
# No -e: a check that finds nothing fails as a mismatch, and the rest run.
set -uo pipefail

usage='usage: switch-check.sh BASELINE ORIGIN [WWW]'
baseline=${1:?$usage}
origin=${2:?$usage}
www=${3:-}
failed=0
check() {
  if [ "$2" = "$3" ]; then echo "ok    $1"; else echo "FAIL  $1: '$2', expected '$3'"; failed=1; fi
}
status() { curl -sS -o /dev/null -w '%{http_code}' "$1"; }
# A response header's value, by its name in lowercase.
header() {
  curl -sS -o /dev/null -D - "$1" | tr -d '\r' |
    awk -v name="$2" 'tolower(substr($0, 1, index($0, ":") - 1)) == name { print substr($0, index($0, ":") + 2); exit }'
}

locs() { curl -sS "$1" | grep -o '<loc>[^<]*</loc>' | sed -E 's#</?loc>##g'; }
paths=$(
  cd "$baseline/site" && find . -name index.html | sed -E 's#^\.##; s#index\.html$##'
  for map in $(locs "$origin/sitemap-index.xml" | sed -E 's#^https?://[^/]+##'); do
    locs "$origin$map" | sed -E 's#^https?://[^/]+##'
  done
  printf '%s\n' /rss.xml /robots.txt /llms.txt /sitemap-index.xml
)
bad=0
count=0
while read -r path; do
  count=$((count + 1))
  code=$(status "$origin$path")
  if [ "$code" != 200 ]; then echo "FAIL  $path: $code"; bad=1; failed=1; fi
done < <(sort -u <<< "$paths")
[ "$bad" = 1 ] || echo "ok    $count URLs of the baseline and the sitemap are 200s"

guids() { grep -o '<guid[^>]*>[^<]*</guid>' | sed -E 's#<[^>]*>##g' | sort; }
check "the feed's guids" "$(curl -sS "$origin/rss.xml" | guids | tr '\n' ' ')" "$(guids < "$baseline/site/rss.xml" | tr '\n' ' ')"

check "Cache-Control on a Page" "$(header "$origin/" cache-control)" "public, max-age=0, must-revalidate"
check "Cache-Control on the feed" "$(header "$origin/rss.xml" cache-control)" "public, max-age=0, must-revalidate"
css=$(curl -sS "$origin/" | grep -o '/assets/[^"]*\.css' | head -1)
check "Cache-Control on $css" "$(header "$origin$css" cache-control)" "public, max-age=31536000, immutable"
check "X-Content-Type-Options" "$(header "$origin/" x-content-type-options)" nosniff
check "HTTP/3 advertised" "$(header "$origin/" alt-svc | grep -o 'h3=' | head -1)" "h3="
check "llms.txt's Content-Type" "$(header "$origin/llms.txt" content-type)" "text/plain; charset=utf-8"
check "robots.txt's sitemap" "$(curl -sS "$origin/robots.txt" | grep '^Sitemap:')" "Sitemap: https://eyuan.me/sitemap-index.xml"
check "a missing URL's status" "$(status "$origin/switch-check-missing/")" 404
check "the 404 Page" "$(curl -sS "$origin/switch-check-missing/" | grep -o '<title>[^<]*</title>')" "<title>Not found | Eyuan</title>"
check "a slashless URL" "$(header "$origin/about" location)" /about/
# Email obfuscation's decoder and Web Analytics' beacon, which the zone
# injects into HTML (#25).
check "Cloudflare's injected scripts" "$(curl -sS "$origin/" "$origin/about/" | grep -oE 'cdn-cgi|cloudflareinsights' | sort -u | tr '\n' ' ')" ""
# Left to the zone (#25): reported, not checked.
echo "info  Strict-Transport-Security: $(header "$origin/" strict-transport-security)"

if [ -n "$www" ]; then
  check "www's status" "$(status "$www/blog/first/?from=www")" 301
  check "www's Location" "$(header "$www/blog/first/?from=www" location)" "$origin/blog/first/?from=www"
  # The zone's Always Use HTTPS: a Workers Custom Domain serves http as
  # it is otherwise. Netlify upgraded it before the switch.
  for host in "$origin" "$www"; do
    check "http on ${host#https://}" "$(status "http://${host#https://}/about/") $(header "http://${host#https://}/about/" location)" "301 $host/about/"
  done
  # The zone's HSTS, as Netlify sent it before the switch (#80).
  check "HSTS on the apex" "$(header "$origin/" strict-transport-security)" "max-age=31536000"
  check "HSTS on www" "$(header "$www/blog/first/" strict-transport-security)" "max-age=31536000"
fi
exit "$failed"
