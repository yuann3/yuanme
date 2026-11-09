# Cutting eyuan.me over to yuanme

Steps 3 to 7 of the cutover plan in [#39](https://github.com/yuann3/yuanme/issues/39), carried out in [#80](https://github.com/yuann3/yuanme/issues/80). The switch was made on 2026-11-09 at 15:28 UTC: `eyuan.me` is served by yuanme from Cloudflare Workers. Each step is written below as the owner approved it, with what happened and how to undo it. Day 28's field metrics and day 30's move to a Custom Domain are [#98](https://github.com/yuann3/yuanme/issues/98).

## Where things are

- **Production:** `eyuan.me` is the Worker `eyuan-me` on account `337851642f9a39a7999e95ed7f339693`, by the Workers Route `eyuan.me/*` (id `a058e0eb39be4c9f9eaedcb0196fc72e`) on the zone's proxied apex CNAME, which still names Netlify. Its deployed version is eyuan.me's `main` at `e703db1`, built by yuanme at `2ccec19`. `www.eyuan.me` is a 301 to the apex from the zone's Redirect Rule.
- **The preview:** the same Worker at <https://eyuan-me.yuann3.workers.dev>; `wrangler.jsonc` sets `"workers_dev": true`.
- **The Astro build on Workers:** version `a9bee5ba-019f-4feb-9684-1a86b4d24e56` of the same Worker is the Parity baseline's `site/`, uploaded for the TTFB comparison. It is also the fastest rollback below.
- **Netlify:** the site `eyuan.netlify.app` still serves its last Astro deploy and gets no new ones. It stays for the 30 days' rollback.
- **Deploys:** `main`'s [`deploy.yml`](https://github.com/yuann3/eyuan.me/blob/main/.github/workflows/deploy.yml) builds yuanme from the commit in `YUANME_REV`, cached per commit, and builds the Site. A push to `main` runs `wrangler deploy`; any other push, or a pull request, runs `wrangler versions upload --preview-alias <branch or pr-N>`, at `https://<alias>-eyuan-me.yuann3.workers.dev` with `X-Robots-Tag: noindex`. The deploy leaves the route alone, since `wrangler.jsonc` doesn't list it, and keeps `workers.dev`. GitHub Actions is locked for a billing issue on the owner's account since 2026-11-06, so until that is fixed nothing deploys from CI: deploy by hand with `yuanme build` and `npx wrangler@4.156.0 deploy`.
- **Secrets:** eyuan.me has `CLOUDFLARE_API_TOKEN`, its own, and `CLOUDFLARE_ACCOUNT_ID`, as well as the old `NETLIFY_*`.

## Readiness (#39 §4)

| Check | Result |
|---|---|
| Parity in Chromium, Firefox and WebKit | 0 differences from the Parity baseline in URLs, the normalized DOM, and pixels and behaviour in each engine: the Parity workflow on `4358dd4`, with the port at `506a3c9` |
| Every Reference site Gate, lab mobile LCP ≤ 2.0 s | **Not met as #39 words it.** See below |
| Firefox click-through | Scripted in Firefox stable 159.0, headed, on the owner's Mac and its GPU (WebGL `Apple M1, or similar`, Firefox's masked name): `/`, the nav, a Project, the Post, Back and Forward, `CLOSE ×`, a 404, and `/`, `/about/` and the Post at 390 px. Every swap kept the Film's canvas, Back and Forward restored the panel's scroll, and there was no console error, failed request or 4xx. The owner's own click-through is still #39's step |
| The feed | `rss.xml` keeps its URL. Its guids are the production feed's strings, `https://www.eyuan.me/blog/<slug>/`, with `isPermaLink="false"` (`rss-guid`) |
| The URL check | Every URL of the production sitemap (27) and every Page and feed of the baseline is a 200 on the preview. Five baseline files are 404s, under `unreferenced-files` (#23): `/plane.png`, `/rustc.PNG`, `/Ylib.png`, Astro's copies of the hero originals, and the two Atkinson `.woff` files. No Page of the port links them |

`parity/switch-check.sh target/baseline https://eyuan-me.yuann3.workers.dev` passes. On `eyuan.me` today it fails on the Astro headers and on the email-obfuscation script, as it should.

### The Gates

From the same run, and the runs of #76 before it:

- **Lab mobile LCP:** `/` 2052 ms and `/about/` 2052 ms, within #15's 10 % tolerance. The Post, `/blog/extending-c-stdlib/`, 2324 ms (2328 and 2556 ms in #76), over 2.2 s. `lab.ts` reports it and doesn't fail on it, until #95.
- **Lab desktop LCP:** `/` 880 ms and `/about/` 844 ms, at most the tolerance; the Post 972 ms, over it (836 to 884 ms in #76). Reported (#95).
- **Peak RSS of a cold build:** 551 MB on CI against 512 MB (524 to 550 MB in #76), report-only (#96); 866 MB on a 16-thread Mac.
- **The other build Gates:** cold build 406 ms, image encoding 50.9 ms an output, warm 56 ms, incremental 29 ms, dev cold start 54 ms, check 48 ms, all within.
- **Render-path requests:** 9 a Page, 10 with `code.css`, against 8: Acknowledged overages in yuanme.toml (#94), so `--strict-budgets` passes.

#39 makes "every Reference site Gate passes, with lab mobile LCP ≤ 2.0 s" a condition of the switch, and names a Post among the Pages. As written, the Post's mobile LCP and desktop LCP (#95) and peak RSS (#96) hold it back. #95 is the shell's first reveal, which is the owner's design call. For that decision, `lab.ts` on one host, the same hour, on the port and on the Parity baseline, which is what `eyuan.me` serves:

| Page | Port mobile | Astro mobile | Port desktop | Astro desktop |
|---|---:|---:|---:|---:|
| `/` | 2068 ms | 2340 ms | 872 ms | 992 ms |
| The Post | 2236 ms | 10,076 ms | 808 ms | 1764 ms |
| `/about/` | 2012 ms | 2320 ms | 788 ms | 924 ms |

CLS is 0 and TBT at most 10 ms on both. Astro's Google Fonts came from the network, not from the baseline's `remote/`.

## Headers and redirects

| | Today: Cloudflare → Netlify | The preview |
|---|---|---|
| Pages, feeds, Static files | `public,max-age=0,must-revalidate` | `public, max-age=0, must-revalidate`, `X-Content-Type-Options: nosniff` |
| Hashed files | `/_astro/`, `max-age=14400` from Browser Cache TTL | `/assets/`, `public, max-age=31536000, immutable` |
| Compression, HTTP/3 | br, `alt-svc: h3` | zstd and br, `alt-svc: h3` |
| A slashless URL, `/about` | 301 | 307 (#25) |
| `/nope/` | 404 with the 404 Page | 404 with the 404 Page |
| `llms.txt` | `text/plain; charset=UTF-8` | `text/plain; charset=utf-8`, from a `[[host.headers]]` rule: Workers sent no charset |
| `robots.txt` | Netlify's | `static/robots.txt`, with the apex sitemap |
| `www.eyuan.me/*` | 301 to `https://eyuan.me/*`, path and query kept, by Netlify | Needs the Redirect Rule below |
| HSTS | `max-age=31536000`, **sent by Netlify** | None |
| Email obfuscation | **On**: `/` and `/about/` load `/cdn-cgi/scripts/…/email-decode.min.js` | Off until the zone serves it |

HSTS doesn't come from the zone today: `docs.eyuan.me`, a Workers Custom Domain on the same zone, sends none. #39's "HSTS stays as the zone sets it today" would drop it at the switch. Browsers that saw it keep it for up to a year.

## TTFB on the same payload

#10's method: Globalping, an HTTPS GET of `/` with `Accept-Encoding: br, gzip`, one probe in each of Frankfurt, London, Virginia Beach, Los Angeles, São Paulo, Johannesburg, Mumbai, Singapore, Tokyo and Sydney, then the same probes again about 2 s later. `firstByte` in ms, median and worst of the 10, on 2026-11-01 from 11:15 to 11:30 UTC. Written down, not gated (#15).

| Host | Payload | 1st request | 2nd request | Measurements |
|---|---|---|---|---|
| Workers, `eyuan-me.yuann3.workers.dev` | yuanme | 43.5 / 273 | 34.5 / 365 | `2OgSKTerKSZWsmXf300021QKA`, `2lMkJttkucD9dfMxL00021QKA` |
| Workers, `eyuan-me.yuann3.workers.dev` | Astro baseline | 46.5 / 196 | 37 / 57 | `2ZMuE7oV8627UBHr400021QKB`, `2K7m0sZyi3etbMEZp00021QKB` |
| Netlify, `eyuan.netlify.app` | Astro | 369.5 / 485 | 309.5 / 576 | `2hltkCm3zX6MJtPcr00021QK9`, `2kqIi7QaggbkKxjnr00021QKA` |
| Cloudflare → Netlify, `eyuan.me` | Astro | 216.5 / 656 | 170.5 / 702 | `2o2bcRMpoOkz6GycM00021QKA`, `2kjSZ4QzpimHFlHuy00021QKA` |
| Workers Custom Domain, `docs.eyuan.me` | the docs site | 41 / 196 | 40.5 / 192 | `2tYJCDmMEighmc4lH00021QKA`, `2UifZpNG9O033EdFJ00021QKA` |

- **The host is what changes TTFB.** The Astro baseline answers in 37 to 47 ms from Workers and 310 to 370 ms from Netlify; yuanme's build on Workers is within 3 ms of it. Cloudflare in front of Netlify still sends every Page to Netlify (`cf-cache-status: DYNAMIC`).
- **Not measured: yuanme's build on Netlify.** It needs a deploy to the Netlify site, which this step doesn't make.
- **Preview URLs aren't the deployed Worker.** The same two builds as preview aliases took 250 to 410 ms on a 1st request and 70 to 280 ms on a 2nd. Measure the deployed version.

## The switch

Each step needs the owner's approval. Run the shell steps in a checkout of eyuan.me with wrangler logged in to the account, and `switch-check.sh` from yuanme with the baseline in `target/baseline` (`parity/fetch.sh target/baseline`).

### 0. Before anything changes

1. **Export the zone's DNS records:** DNS › Records › Export, and keep the file. The rollback restores the `eyuan.me` record from it. This step can't read them: the wrangler login has `zone:read` only. The owner's export, on 2026-11-06, before the switch; these are the rollback records:

   | Name | Type | Content | Proxy | TTL |
   |---|---|---|---|---|
   | `eyuan.me` | CNAME | `apex-loadbalancer.netlify.com` | Proxied | Auto |
   | `www` | CNAME | `apex-loadbalancer.netlify.com` | Proxied | Auto |
2. **Secrets.** In Cloudflare, create a token for eyuan.me alone, not a copy of yuanme's: *Account › Workers Scripts › Edit* and *Account › Account Settings › Read*, on this account only. It needs no zone permission while the Custom Domain is kept out of `wrangler.jsonc`, as below. Then:
   ```sh
   gh secret set CLOUDFLARE_API_TOKEN -R yuann3/eyuan.me
   gh secret set CLOUDFLARE_ACCOUNT_ID -R yuann3/eyuan.me --body 337851642f9a39a7999e95ed7f339693
   ```
   The next push to `yuanme` uploads a preview at `yuanme-eyuan-me.yuann3.workers.dev`. Undo: `gh secret delete`, and roll the token.
3. **Tag the last Astro commit:**
   ```sh
   git tag astro-final 1fcd0c5 && git push origin astro-final
   ```

### 1. Zone settings

None of these breaks the Netlify origin, so they can go first, one at a time:

| Setting | Where | Change | Undo |
|---|---|---|---|
| Browser Cache TTL | Caching › Configuration | *Respect Existing Headers* | The previous value, 4 hours |
| Email Address Obfuscation | Scrape Shield | Off. Netlify's Pages then keep their `mailto:` links as written | On |
| Web Analytics | Analytics › Web Analytics | No automatic beacon for `eyuan.me`. None is injected today | — |
| HSTS | SSL/TLS › Edge Certificates › HSTS | **Owner's call.** To keep today's header: on, max-age 12 months, no `includeSubDomains`, no preload, No-Sniff off. It then covers `docs.eyuan.me` too | Off. Browsers keep what they saw for its max-age |
| `www` | Rules › Redirect Rules | A rule: hostname equals `www.eyuan.me`, dynamic redirect to `concat("https://eyuan.me", http.request.uri.path)`, 301, query string kept. The `www` record stays proxied, so the rule answers before Netlify would | Delete the rule: Netlify redirects `www` again |
| Always Use HTTPS | SSL/TLS › Edge Certificates | On | Off: Netlify upgrades `http://` again while it serves |

Check after each: `parity/switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me` keeps passing its `www` lines, and the injected-scripts line passes once obfuscation is off. A `www` 301 that carries `x-nf-request-id` and the body `Redirecting to …` is Netlify's, not the rule's: the rule answers at Cloudflare before the request reaches Netlify.

Checked on 2026-11-06 from 04:37 to 04:42 UTC, after the owner set step 1 in the dashboard:

- **Browser Cache TTL: in effect.** `robots.txt` and `/_astro/` now pass Netlify's `max-age=0` through, where the zone sent `max-age=14400` on 2026-11-01.
- **`www` Redirect Rule: not in effect.** `https://www.eyuan.me/<random path>/` is a 301 from Netlify, with `x-nf-request-id` and Netlify's body, five minutes running.
- **Email obfuscation: still on.** Uncached `/about/` still loads `email-decode.min.js` and has two `/cdn-cgi/l/email-protection` links.
- **HSTS:** can't be told apart while Netlify sends the same header; `switch-check.sh` reports it after the switch.

Checked again on 2026-11-09 at 14:53 and 14:55 UTC, after the owner fixed the rest:

- **`www` Redirect Rule: in effect.** It is a wildcard Single Redirect, `https://www.eyuan.me/*` to `https://eyuan.me/${1}`, 301, query string kept. `https://www.eyuan.me/zz-<random>/?x=1` is a 301 to `https://eyuan.me/zz-<random>/?x=1` from Cloudflare, with no `x-nf-request-id`.
- **Email obfuscation: off.** Uncached `/about/` has no `email-decode.min.js` and no `/cdn-cgi/l/email-protection` link.
- **Always Use HTTPS: on.** It wasn't in the plan, and was off: Netlify upgraded `http://` itself, and a Workers Custom Domain would have served plain `http://` on the apex. Now `http://eyuan.me/about/` and `http://www.eyuan.me/blog/` are 301s to `https://` from Cloudflare, with no `x-nf-request-id`. `switch-check.sh` checks it when given `WWW`.
- **HSTS: not in effect.** No response Cloudflare makes itself sends `Strict-Transport-Security`: not the `www` rule's 301, and not `docs.eyuan.me`, a Workers Custom Domain on the same zone, on a cached 200 or an uncached 404. Only Netlify's responses do. That is how to check it before the switch: `curl -sI https://docs.eyuan.me/` shows `strict-transport-security: max-age=31536000` once the zone sends it. The owner saved it, and at 15:17 UTC `docs.eyuan.me` and the `www` rule's 301 both sent `strict-transport-security: max-age=31536000`. `switch-check.sh` checks it on the apex and `www` when given `WWW`.

### 2. Merge, which deploys to `workers.dev` only

```sh
git switch main && git pull
git merge --no-ff yuanme -m "Build eyuan.me with yuanme and deploy it to Cloudflare Workers"
git push origin main
```

The merge is clean. Its push runs `deploy.yml` with `wrangler deploy`: the Worker's deployed version becomes `main`, still only at `workers.dev`, and Netlify stops getting deploys while it keeps serving `eyuan.me`. Then:

```sh
parity/switch-check.sh target/baseline https://eyuan-me.yuann3.workers.dev
```

This runs #39's steps 3 and 2 in that order, so the Worker's deployed version is checked before `eyuan.me` points at it. Undo: `git revert -m 1 <merge>` and push, which brings back the Netlify workflow and deploys Astro to Netlify again.

Done on 2026-11-09. The merge, `8889762`, was pushed to eyuan.me's `main` at 14:54 UTC; it brings in `main`'s `CONTEXT.md` and changes no output. Its Deploy run didn't start: GitHub Actions is locked for a billing issue on the owner's account, for eyuan.me and yuanme alike, since 2026-11-06. So `main` was built here with yuanme `2ccec19` (`yuanme build --clean --strict-budgets`: 30 Pages, 0 warnings) and deployed with `npx wrangler@4.156.0 deploy`, no domain, at 14:56 UTC: version `12eb7f50-8ade-4860-a49c-77ad8fef9e05`. On it, `switch-check.sh` against workers.dev passed every check, and the parity test on its `dist/` found 0 differences: in URLs and the normalized DOM, and in pixels and behaviour in Chromium 156, Firefox 157 and WebKit 27.2, run here in Playwright 1.64.1's Linux container from 14:57 to 15:09 UTC in place of CI's. Until billing is fixed, a push to `main` deploys nothing, and Netlify keeps serving its last Astro deploy.

### 3. Send `eyuan.me` to the Worker: the switch

#39 attaches `eyuan.me` as a Custom Domain. That was refused, so for the 30 days a Workers Route does it instead, and the Custom Domain waits for day 30.

**The Custom Domain, refused.** `npx wrangler@4.156.0 deploy --domain eyuan.me` was tried on 2026-11-09 at 15:18:55 UTC, from `main`'s build, and the API refused it, although wrangler sends `override_existing_dns_record` outside a terminal: `Hostname 'eyuan.me' already has externally managed DNS records (A, CNAME, etc). Delete them first or try a different hostname. [code: 100117]`. Nothing that `eyuan.me` serves changed. One side effect: given `--domain`, wrangler turns the Worker's `workers.dev` route off unless `wrangler.jsonc` sets `workers_dev`, and keeps that change when the domain fails. `workers.dev` answered 404 from 15:19 until `wrangler deploy` with no domain put it back at 15:19:56 (version `0c0be9be-2abb-41e1-99f8-2e082b22dc66`); a few edge-cached 404s cleared within a minute, and `switch-check.sh` passed on it again at 15:21. A Custom Domain needs the apex CNAME deleted first, with `eyuan.me` unanswered for the seconds in between, and an undo that recreates it the same way.

**The route, the owner's choice.** A Workers Route on the proxied CNAME sends every apex request to the Worker before it reaches Netlify, with no DNS change, and deleting it undoes it at once. Added through the API with wrangler's login, which has *Workers Routes: Edit*, at 15:28:00 UTC, after HSTS was checked on `docs.eyuan.me` and `a9bee5ba` on the Worker:

```sh
curl -X POST -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  https://api.cloudflare.com/client/v4/zones/b704f11be800cc7d6f5aa6f97f6d6ab5/workers/routes \
  --data '{"pattern":"eyuan.me/*","script":"eyuan-me"}'
```

It answered in seconds: a request at 15:28:09 had the Worker's headers and Netlify's body, and from 15:28:19 every request was the Worker's. No `www` route: the Redirect Rule answers `www` first. The route isn't in `wrangler.jsonc`, since a deploy would then need a zone permission the deploy token doesn't have; a deploy leaves routes it doesn't list alone. `"workers_dev": true` in `wrangler.jsonc` (`e703db1`) keeps the preview up. A `wrangler deploy` from that tree at 15:43, as CI would run it, left the route and `workers.dev` in place: version `c9dabf17-c872-4f43-b840-9a7c155200d6`.

### 4. Right after

```sh
parity/switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me
curl -sI https://eyuan.me/ | grep -iE 'x-nf-request-id|strict-transport'
```

`switch-check.sh` covers `/`, the Post, `/about/` and every other Page, `/rss.xml`, a 404, the `www` redirect, `Cache-Control` on Pages and `/assets/`, `alt-svc: h3`, http to https and HSTS on both hosts. `curl` should print no `x-nf-request-id`, Netlify's.

At 15:38 UTC, `switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me` passed all 20 checks, and `/`, `/about/`, the Post, `/rss.xml`, a 404 and `/llms.txt` carried no `x-nf-request-id`. Then one lab check on `eyuan.me` itself, such as PageSpeed Insights on the Post, and resubmit `https://eyuan.me/sitemap-index.xml` in Search Console (#39 §7, day 0).

## Rollback

Netlify stays deployable for 30 days: its site, its secrets, the `eyuan.me` domain on it, and `astro-final` with the old `deploy.yml`. Its certificate for `eyuan.me` and `www` runs to 2027-01-18, past the 30 days, though it can't renew while the domain points at Workers. Roll back for a broken Page or feed, or a failed check; fix forward when it can be fixed within a day. Hashed `/assets/` and Astro's `/_astro/` never share a name, so caches from both sides can live together.

1. **In seconds, with no DNS change:** serve the Astro build from the same Worker.
   ```sh
   npx wrangler@4.156.0 versions deploy a9bee5ba-019f-4feb-9684-1a86b4d24e56@100% -y
   ```
   It is the Parity baseline, the same Pages as Netlify's, with Workers' default headers. Back: `npx wrangler versions deploy <the yuanme version>@100% -y`.
2. **Back to Netlify:** delete the route, and Netlify answers `eyuan.me` again within seconds, by the CNAME that never changed:
   ```sh
   curl -X DELETE -H "Authorization: Bearer $TOKEN" \
     https://api.cloudflare.com/client/v4/zones/b704f11be800cc7d6f5aa6f97f6d6ab5/workers/routes/a058e0eb39be4c9f9eaedcb0196fc72e
   ```
   Or the `eyuan-me` Worker › Settings › Domains & Routes › the route › Delete. This needs no GitHub Actions and no DNS change: Netlify keeps serving its last Astro deploy. The zone settings can stay; Netlify sent the same HSTS and upgraded `http://` itself. Only if that deploy is gone does `gh workflow run deploy.yml -R yuann3/eyuan.me --ref astro-final` put Astro back on Netlify, which needs Actions.

After day 30, with a Custom Domain, the way back is to remove it and recreate the apex record from the export: CNAME to `apex-loadbalancer.netlify.com`, proxied, TTL Auto. `eyuan.me` has no record between the two.

## After the switch

- **Day 28:** read CrUX's field LCP, CLS and INP for `eyuan.me`, with the back/forward CLS kept in #23, and post them on the map.
- **Day 30, 2026-12-09:** delete the Netlify site and its `NETLIFY_*` secrets, and archive the Astro deploy workflow. Then move to the Custom Domain, which the apex CNAME blocks: delete the CNAME, add the Custom Domain `eyuan.me` to `eyuan-me` at once, and delete the route. To keep it, list it in `wrangler.jsonc` (`"routes": [{ "pattern": "eyuan.me", "custom_domain": true }]`, as `site/wrangler.jsonc` has `docs.eyuan.me`) with `"workers_dev": true` kept, and give the deploy token *Zone › Workers Routes › Edit* on `eyuan.me`. Then run `switch-check.sh` again. Each needs the owner.
- **`workers.dev`:** it serves the same Pages with no `noindex`. `"workers_dev": false` with `"preview_urls": true` in `wrangler.jsonc` turns it off and keeps the previews, once nothing needs the preview.
- **GitHub Actions:** the owner's billing lock stops eyuan.me's deploys and yuanme's CI alike. Once it is lifted, re-run yuanme's CI and Parity on `main`.
- **The guide:** the "Migrate from Astro" guide for the docs site (#38), from the port's notes.
