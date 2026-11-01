# Cutting eyuan.me over to yuanme

Steps 3 to 7 of the cutover plan in [#39](https://github.com/yuann3/yuanme/issues/39), carried out in [#80](https://github.com/yuann3/yuanme/issues/80). Everything up to the switch is done; each step that changes what `eyuan.me` or `www.eyuan.me` serves waits for the owner and is written below as commands, with how to undo it. State as of 2026-11-01.

## Where things are

- **Production:** `eyuan.me` is Astro on Netlify (`eyuan.netlify.app`), behind proxied records in the Cloudflare zone `eyuan.me`. eyuan.me's `main` is `1fcd0c5`; its `deploy.yml` deploys to Netlify.
- **The preview:** the Worker `eyuan-me` on account `337851642f9a39a7999e95ed7f339693`, at <https://eyuan-me.yuann3.workers.dev>. Its deployed version is the `yuanme` branch at `506a3c9`, built by yuanme at `2ccec19`. It has no route or Custom Domain.
- **The Astro build on Workers:** version `a9bee5ba-019f-4feb-9684-1a86b4d24e56` of the same Worker is the Parity baseline's `site/`, uploaded for the TTFB comparison. It is also the fastest rollback below.
- **Deploys:** the `yuanme` branch's [`deploy.yml`](https://github.com/yuann3/eyuan.me/blob/yuanme/.github/workflows/deploy.yml) builds yuanme from the commit in `YUANME_REV`, cached per commit, and builds the Site. A push to `main` runs `wrangler deploy`. A push to any other branch, or a pull request, runs `wrangler versions upload --preview-alias <branch or pr-N>`, which leaves the deployed version alone; `yuanme` previews at <https://yuanme-eyuan-me.yuann3.workers.dev>, which sends `X-Robots-Tag: noindex`. So nothing reaches the deployed version until the owner merges into `main`, and nothing reaches `eyuan.me` until the Custom Domain is attached.
- **Secrets:** eyuan.me has only `NETLIFY_AUTH_TOKEN` and `NETLIFY_SITE_ID`, so the workflow builds and deploys nothing yet. It needs `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID`.

## Readiness (#39 §4)

| Check | Result |
|---|---|
| Parity in Chromium, Firefox and WebKit | URLs and the normalized DOM: 0 differences. Pixels and behaviour: the Parity workflow's `port` jobs on the pinned commit |
| Every Reference site Gate, lab mobile LCP ≤ 2.0 s | **Not met as #39 words it.** See below |
| Firefox click-through | Scripted in Firefox stable 159.0, headed, on the owner's Mac and its GPU (WebGL `Apple M1, or similar`, Firefox's masked name): `/`, the nav, a Project, the Post, Back and Forward, `CLOSE ×`, a 404, and `/`, `/about/` and the Post at 390 px. Every swap kept the Film's canvas, Back and Forward restored the panel's scroll, and there was no console error, failed request or 4xx. The owner's own click-through is still #39's step |
| The feed | `rss.xml` keeps its URL. Its guids are the production feed's strings, `https://www.eyuan.me/blog/<slug>/`, with `isPermaLink="false"` (`rss-guid`) |
| The URL check | Every URL of the production sitemap (27) and every Page and feed of the baseline is a 200 on the preview. Five baseline files are 404s, under `unreferenced-files` (#23): `/plane.png`, `/rustc.PNG`, `/Ylib.png`, Astro's copies of the hero originals, and the two Atkinson `.woff` files. No Page of the port links them |

`parity/switch-check.sh target/baseline https://eyuan-me.yuann3.workers.dev` passes. On `eyuan.me` today it fails on the Astro headers and on the email-obfuscation script, as it should.

### The Gates

From the Parity workflow on `2ccec19`, with the port at `77b326d`; the pin's change since touches no output but `_headers`:

- **Lab mobile LCP:** `/` 2056 ms and `/about/` 2072 ms, within #15's 10 % tolerance. The Post, `/blog/extending-c-stdlib/`, 2328 ms (2556 ms on an earlier run), over 2.2 s. `lab.ts` reports it and doesn't fail on it, until #95.
- **Lab desktop LCP:** 836 to 884 ms against 800 ms, reported (#95).
- **Peak RSS of a cold build:** 524 to 550 MB on CI against 512 MB, report-only (#96); 866 MB on 16 threads.
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

Each step is for the owner. Run the shell steps in a checkout of eyuan.me with wrangler logged in to the account, and `switch-check.sh` from yuanme with the baseline in `target/baseline` (`parity/fetch.sh target/baseline`).

### 0. Before anything changes

1. **Export the zone's DNS records:** DNS › Records › Export, and keep the file. The rollback restores the `eyuan.me` record from it. This step can't read them: the wrangler login has `zone:read` only.
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

Check after each: `parity/switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me` keeps passing its `www` lines, and the injected-scripts line passes once obfuscation is off.

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

### 3. Attach `eyuan.me`: the switch

The Custom Domain is attached by hand and kept out of `wrangler.jsonc` for the 30 days: wrangler only touches Custom Domains that its config lists, so CI deploys keep it, and a rollback can't be undone by the next push.

**Dashboard:** Workers & Pages › `eyuan-me` › Settings › Domains & Routes › Add › Custom domain › `eyuan.me`. Cloudflare's docs say a Custom Domain can't be made on a hostname with a CNAME record, and their way is to delete the record first: that leaves `eyuan.me` without a record for the seconds in between.

**Or wrangler, which needs no gap:** wrangler 4.156.0 asks the API for the Custom Domain's changeset, lists the conflicting record, and on a terminal asks "Update them to point to this script instead?"; yes replaces it in one step (`override_existing_dns_record`). Outside a terminal, as in CI, it replaces the record without asking. From the merged `main`:

```sh
npx wrangler@4.156.0 deploy --domain eyuan.me
```

It deploys the local `dist/`, so build it first with the yuanme CI used: `cargo install --locked --git https://github.com/yuann3/yuanme --rev 2ccec19 yuanme`, then `yuanme build`.

Neither path was tried here: trying it needs a DNS change. To rehearse, the owner can make a proxied `rehearsal.eyuan.me` CNAME to `eyuan.netlify.app` and run the same command with `--domain rehearsal.eyuan.me`, then remove both.

### 4. Right after

```sh
parity/switch-check.sh target/baseline https://eyuan.me https://www.eyuan.me
curl -sI https://eyuan.me/ | grep -iE 'x-nf-request-id|strict-transport'
```

`switch-check.sh` covers `/`, the Post, `/about/` and every other Page, `/rss.xml`, a 404, the `www` redirect, `Cache-Control` on Pages and `/assets/`, and `alt-svc: h3`. `curl` should print no `x-nf-request-id`, Netlify's, and shows the HSTS decision. Then one lab check on `eyuan.me` itself, such as PageSpeed Insights on the Post, and resubmit `https://eyuan.me/sitemap-index.xml` in Search Console (#39 §7, day 0).

## Rollback

Netlify stays deployable for 30 days: its site, its secrets, the `eyuan.me` domain on it, and `astro-final` with the old `deploy.yml`. Its certificate for `eyuan.me` and `www` runs to 2027-01-18, past the 30 days, though it can't renew while the domain points at Workers. Roll back for a broken Page or feed, or a failed check; fix forward when it can be fixed within a day. Hashed `/assets/` and Astro's `/_astro/` never share a name, so caches from both sides can live together.

1. **In seconds, with no DNS change:** serve the Astro build from the same Worker.
   ```sh
   npx wrangler@4.156.0 versions deploy a9bee5ba-019f-4feb-9684-1a86b4d24e56@100% -y
   ```
   It is the Parity baseline, the same Pages as Netlify's, with Workers' default headers. Back: `npx wrangler versions deploy <the yuanme version>@100% -y`.
2. **Back to Netlify, #39's rollback:** remove the Custom Domain (the `eyuan-me` Worker › Domains & Routes › `eyuan.me` › Remove), which deletes its record, and at once recreate the `eyuan.me` record from the export, proxied. Between the two, `eyuan.me` has no record. Then turn back the zone settings that need it. If `main` deployed since, `gh workflow run deploy.yml -R yuann3/eyuan.me --ref astro-final` puts Astro back on Netlify.

## After the switch

- **Day 28:** read CrUX's field LCP, CLS and INP for `eyuan.me`, with the back/forward CLS kept in #23, and post them on the map.
- **Day 30:** delete the Netlify site and its secrets, archive the Astro deploy workflow, and move the Custom Domain into `wrangler.jsonc` (`"routes": [{ "pattern": "eyuan.me", "custom_domain": true }]`, as `site/wrangler.jsonc` has `docs.eyuan.me`); the token then needs *Zone › Workers Routes › Edit* on `eyuan.me`. Each needs the owner.
- **`workers.dev`:** it serves the same Pages with no `noindex`. `"workers_dev": false` with `"preview_urls": true` in `wrangler.jsonc` turns it off and keeps the previews.
- **The guide:** the "Migrate from Astro" guide for the docs site (#38), from the port's notes.
