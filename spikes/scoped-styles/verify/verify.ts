// Browser check of the scoped-styles spike. Chromium via Playwright, JavaScript disabled in the pages.
//   bun verify/verify.ts <astro-dist dir>
// Serves out/site (ours; real Reference global CSS at /_ref/global.css) and the Reference's Astro build,
// then compares what each scoped rule matches.
import { chromium } from "playwright";
import { readFileSync, existsSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const root = join(import.meta.dir, "..");
const ours = join(root, "out/site");
const astro = process.argv[2];
const manifest = JSON.parse(readFileSync(join(ours, "manifest.json"), "utf8"));
const attrToTpl: Record<string, string> = Object.fromEntries(Object.entries(manifest.attrs).map(([t, a]) => [a as string, t]));
const cidToTpl: Record<string, string> = {
  "data-astro-cid-bvzihdzo": "layouts/post.html", "data-astro-cid-uogrpsp5": "projects/entry.html", "data-astro-cid-2hwget37": "projects/index.html",
  "data-astro-cid-jqm7f7gd": "components/list_row.html", "data-astro-cid-d776pwuy": "components/pagination.html", "data-astro-cid-lp6sgjcj": "components/photo_pile.html",
};
const globalCss = ["_slug_.ynAfo1z9.css", "about.DhiEpsgV.css"].map((f) => readFileSync(join(astro, "_astro", f), "utf8")).join("\n");
const mime: Record<string, string> = { html: "text/html; charset=utf-8", css: "text/css", js: "text/javascript", json: "application/json" };
function serve(dir: string, extra: Record<string, string> = {}) {
  return Bun.serve({ port: 0, fetch(req) {
    const p = decodeURIComponent(new URL(req.url).pathname);
    if (extra[p]) return new Response(extra[p], { headers: { "content-type": "text/css" } });
    let f = join(dir, p);
    if (existsSync(f) && statSync(f).isDirectory()) f = join(f, "index.html");
    if (!existsSync(f)) return new Response("nf", { status: 404 });
    return new Response(readFileSync(f), { headers: { "content-type": mime[f.split(".").pop()!] ?? "application/octet-stream" } });
  } });
}
const sOurs = serve(ours, { "/_ref/global.css": globalCss });
const sAstro = serve(astro);

// ---- runs in the page
function collect(prefix: string) {
  const attrRe = new RegExp("\\[(" + prefix + "[a-z0-9]+)\\]", "g");
  const splitTop = (s: string) => { const out: string[] = []; let d = 0, cur = ""; for (const ch of s) { if (ch === "(" || ch === "[") d++; if (ch === ")" || ch === "]") d--; if (ch === "," && d === 0) { out.push(cur.trim()); cur = ""; } else cur += ch; } out.push(cur.trim()); return out; };
  // canonical text: simple selectors of each compound sorted, attribute replaced by [S]
  const canon = (s: string) => {
    const toks: string[] = []; let d = 0, cur = "";
    const flush = () => { if (cur) { toks.push(cur); cur = ""; } };
    for (let i = 0; i < s.length; i++) { const ch = s[i];
      if (d === 0 && (ch === " " || ch === ">" || ch === "+" || ch === "~")) { flush(); if (ch !== " ") toks.push(ch); else if (toks.length && toks[toks.length - 1] !== " " ) toks.push(" "); continue; }
      if (ch === "(" || ch === "[") d++; if (ch === ")" || ch === "]") d--; cur += ch; }
    flush();
    const clean = toks.filter((t, i) => !(t === " " && ([">", "+", "~"].includes(toks[i + 1]) || [">", "+", "~"].includes(toks[i - 1]))));
    return clean.map((t) => { if ([" ", ">", "+", "~"].includes(t)) return t;
      const parts: string[] = []; let dd = 0, c = "";
      for (let i = 0; i < t.length; i++) { const ch = t[i]; if (dd === 0 && (ch === "." || ch === "[" || ch === ":" || ch === "#") && c && !(ch === ":" && c === ":")) { parts.push(c); c = ""; } if (ch === "(" || ch === "[") dd++; if (ch === ")" || ch === "]") dd--; c += ch; }
      if (c) parts.push(c);
      const pe = parts.filter((p) => p.startsWith("::")); const rest = parts.filter((p) => !p.startsWith("::")).sort();
      return rest.join("") + pe.join(""); }).join("");
  };
  const sig = (el: Element) => el.tagName.toLowerCase() + [...el.classList].sort().map((c) => "." + c).join("");
  const info = (el: Element) => ({ sig: sig(el), md: !!el.closest("[data-md]"), src: el.getAttribute("data-src") });
  const rules: any[] = [];
  const walk = (list: CSSRuleList, ctx: string) => { for (const r of list as any) {
    if (r instanceof CSSStyleRule) { if (r.selectorText.includes(prefix)) for (const one of splitTop(r.selectorText)) {
        const scope = [...one.matchAll(attrRe)].map((m) => m[1]);
        const compounds = one.replace(/::?[a-z-]+$/, "");
        const last = compounds.split(/[\s>+~]+(?![^(\[]*[)\]])/).pop()!;
        let matched: Element[] = []; let unscoped: Element[] = [];
        const q = one.replace(/::[-a-z]+$/, ""); // querySelectorAll never returns pseudo-elements
        try { matched = [...document.querySelectorAll(q)]; unscoped = [...document.querySelectorAll(q.replace(attrRe, ":not([data-zz])"))]; } catch (e) { matched = []; }
        rules.push({ scope: scope[0], scopes: [...new Set(scope)], ctx, raw: one, norm: canon(one.replace(attrRe, "[S]")), subjectScoped: attrRe.test(last) || (attrRe.lastIndex = 0, attrRe.test(last)),
          n: matched.length, hits: matched.map(info), unscopedN: unscoped.length, leaked: unscoped.filter((e) => !matched.includes(e)).map(info) });
        attrRe.lastIndex = 0;
      }
      if (r.cssRules?.length) walk(r.cssRules, ctx); }
    else if (r.cssRules) walk(r.cssRules, (ctx + " @" + (r instanceof CSSMediaRule ? "media " + r.conditionText.replace(/\(min-width: /g, "(width >= ") : r instanceof CSSContainerRule ? "container " + (r as any).conditionText.replace(/\(min-width: /g, "(width >= ") : r.constructor.name)).trim()); } };
  for (const sh of document.styleSheets as any) { try { walk(sh.cssRules, ""); } catch (e) {} }
  const stamped: Record<string, any[]> = {};
  for (const el of document.querySelectorAll("*")) for (const a of el.attributes) if (a.name.startsWith(prefix)) (stamped[a.name] ??= []).push(info(el));
  return { rules, stamped };
}

const browser = await chromium.launch();
const ctx = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 1280, height: 900 } });
const page = await ctx.newPage();
const out: string[] = [];
const log = (s = "") => { out.push(s); console.log(s); };
const count = (xs: any[]) => { const m: Record<string, number> = {}; for (const x of xs) m[x.sig] = (m[x.sig] ?? 0) + 1; return m; };
const setEq = (a: string[], b: string[]) => a.length === b.length && a.every((x) => b.includes(x));
let failures = 0;
const ok = (cond: boolean, msg: string) => { if (!cond) failures++; log(`${cond ? "PASS" : "FAIL"}  ${msg}`); };

// ---- 1. our pages: provenance invariants
log("## 1. Our render: provenance (every page)\n");
let tot = { rules: 0, matches: 0, foreign: 0, md: 0, leakedPrevented: 0, leakedMd: 0, globalHits: 0, stampedMd: 0, stampedWrong: 0, stamped: 0 };
const ourRules: Record<string, any> = {};
const oursByPage: Record<string, any> = {};
for (const url of manifest.pages as string[]) {
  await page.goto(`http://localhost:${sOurs.port}${url}`);
  const r = await page.evaluate(collect, "data-y-");
  oursByPage[url] = r;
  for (const [attr, els] of Object.entries(r.stamped) as any) for (const e of els) { tot.stamped++; if (e.md) tot.stampedMd++; if (e.src !== attrToTpl[attr]) tot.stampedWrong++; }
  for (const rule of r.rules) {
    const tpl = attrToTpl[rule.scope]; const key = `${tpl} |${rule.ctx}| ${rule.norm}`;
    const agg = (ourRules[key] ??= { tpl, n: 0, subjectScoped: rule.subjectScoped, leaked: 0, leakedMd: 0, mdHits: 0, foreign: 0, raw: rule.raw });
    agg.n += rule.n; agg.leaked += rule.leaked.length; agg.leakedMd += rule.leaked.filter((e: any) => e.md).length;
    tot.matches += rule.n; tot.leakedPrevented += rule.leaked.length; tot.leakedMd += rule.leaked.filter((e: any) => e.md).length;
    for (const h of rule.hits) { if (rule.subjectScoped) { if (h.md) { tot.md++; agg.mdHits++; } if (h.src !== tpl) { tot.foreign++; agg.foreign++; } } else { tot.globalHits++; if (h.md) agg.mdHits++; } }
  }
}
tot.rules = Object.keys(ourRules).length;
log(`${manifest.pages.length} pages, ${tot.rules} distinct scoped selectors, ${tot.matches} selector-element matches, ${tot.stamped} stamped elements.\n`);
ok(tot.stampedWrong === 0, `every element carrying data-y-<id> was written in that id's template (${tot.stampedWrong} wrong)`);
ok(tot.stampedMd === 0, `no element of rendered content (data-md) carries a scope attribute (${tot.stampedMd})`);
ok(tot.foreign === 0, `(a) scoped-subject selectors match only elements of their own template (${tot.foreign} foreign matches)`);
ok(tot.md === 0, `(b) scoped-subject selectors match no rendered-content / slot element (${tot.md})`);
log(`      the same selectors without the attribute would also have matched ${tot.leakedPrevented} elements (${tot.leakedMd} of them inside rendered content): these are the leaks scoping prevents.`);
const globals = Object.entries(ourRules).filter(([, v]: any) => !v.subjectScoped);
log(`      :global() subjects (${globals.length} selectors) matched ${tot.globalHits} elements; these are meant to reach content:`);
for (const [k, v] of globals as any) log(`        ${k}  -> ${v.n} matches, ${v.mdHits} in content, ${v.leaked} more without the ancestor attribute`);
const dead = Object.entries(ourRules).filter(([, v]: any) => v.n === 0).map(([k]) => k);
log(`\nselectors with no match on any page (${dead.length}); all need runtime state set by PhotoPile's script:`);
for (const d of dead) log(`  ${d}`);
ok(dead.every((d) => /data-(st|press|drag|ready)|:hover|:focus/.test(d)), "every unmatched selector is state-dependent");

// ---- 2. against the Reference's Astro build
log("\n## 2. Against the Reference's Astro build\n");
const pairs: [string, string][] = [["/blog/", "/blog/"], ["/about/", "/about/"], ["/projects/", "/projects/"], ["/projects/lrle/", "/projects/lrle/"], ["/blog/extending-c-stdlib/", "/blog/extending_c_stdlib/"]];
const astroRules: Record<string, any> = {};
for (const [au, ou] of pairs) {
  await page.goto(`http://localhost:${sAstro.port}${au}`);
  const a = await page.evaluate(collect, "data-astro-cid-");
  const o = oursByPage[ou];
  log(`### ${au}`);
  const aSt: Record<string, any[]> = {}; for (const [k, v] of Object.entries(a.stamped)) aSt[cidToTpl[k] ?? k] = v as any[];
  const oSt: Record<string, any[]> = {}; for (const [k, v] of Object.entries(o.stamped)) oSt[attrToTpl[k]] = v as any[];
  for (const tpl of new Set([...Object.keys(aSt), ...Object.keys(oSt)])) {
    const ac = count(aSt[tpl] ?? []), oc = count(oSt[tpl] ?? []);
    const onlyA = Object.keys(ac).filter((k) => !(k in oc)), onlyO = Object.keys(oc).filter((k) => !(k in ac));
    const na = (aSt[tpl] ?? []).length, no = (oSt[tpl] ?? []).length;
    log(`  stamped elements, ${tpl}: Astro ${na}, ours ${no}; kinds only in Astro: ${JSON.stringify(onlyA.map((k) => `${k} x${ac[k]}`))}; only in ours: ${JSON.stringify(onlyO.map((k) => `${k} x${oc[k]}`))}`);
  }
  const ar: Record<string, any> = {}; for (const r of a.rules) { const k = `${cidToTpl[r.scope]} |${r.ctx}| ${r.norm}`; ar[k] = r; astroRules[k] = r; }
  const or_: Record<string, any> = {}; for (const r of o.rules) or_[`${attrToTpl[r.scope]} |${r.ctx}| ${r.norm}`] = r;
  let same = 0, diff = 0, sameCount = 0, cmp = 0, idle = 0;
  for (const [k, r] of Object.entries(ar)) {
    const mine = or_[k]; if (!mine) { log(`  rule missing in ours: ${k}`); diff++; continue; }
    if (!r.subjectScoped) continue;
    if (r.n === 0 || mine.n === 0) { idle++; continue; } // not exercised on this pair of pages (state selectors; no pagination on the Reference's /blog/)
    cmp++;
    const as = Object.keys(count(r.hits)), os = Object.keys(count(mine.hits));
    if (setEq(as, os)) same++; else { diff++; log(`  DIFF ${k}: Astro ${JSON.stringify(count(r.hits))} ours ${JSON.stringify(count(mine.hits))}`); }
    if (r.n === mine.n) sameCount++;
  }
  ok(diff === 0, `${au}: ${same}/${cmp} scoped-subject rules of this page match the same kinds of element as in Astro's output (${sameCount} with the same count; ${idle} more not exercised on one side)`);
}
const aKeys = Object.keys(astroRules), oKeys = Object.keys(ourRules);
const missing = aKeys.filter((k) => !oKeys.includes(k)), extra = oKeys.filter((k) => !aKeys.includes(k));
ok(missing.length === 0 && extra.length === 0, `selector sets equal after normalising the attribute name and simple-selector order: Astro ${aKeys.length}, ours ${oKeys.length}`);
for (const k of missing) log(`  only in Astro: ${k}  (${astroRules[k].raw})`);
for (const k of extra) log(`  only in ours: ${k}  (${ourRules[k].raw})`);

// ---- 3. specificity dependencies
log("\n## 3. Specificity dependencies (computed style, Chromium)\n");
const cs = (sel: string, props: string[]) => page.evaluate(([s, ps]) => { const el = document.querySelector(s as string); if (!el) return null; const c = getComputedStyle(el); return Object.fromEntries((ps as string[]).map((p) => [p, c.getPropertyValue(p)])); }, [sel, props] as const);
const unscopedCss = readFileSync(join(ours, "scoped.css"), "utf8").replace(/\[data-y-[a-z0-9]+\]/g, "");
async function probe(base: string, reversed: boolean, control = false) {
  const res: Record<string, any> = {};
  const go = async (url: string, w: number) => { await page.setViewportSize({ width: w, height: 900 }); await page.goto(base + url);
    if (reversed) await page.evaluate(() => { const g = document.querySelector("link[data-global]")!; document.head.appendChild(g); });
    if (control) await page.evaluate((css) => { const st = document.createElement("style"); st.textContent = css; document.querySelector("link[data-scoped]")!.replaceWith(st); }, unscopedCss); await page.waitForTimeout(reversed ? 150 : 0); };
  await go("/projects/", 375);
  res[".project @375 (scoped align-items:flex-start vs .row center)"] = await cs("a.project", ["align-items", "gap", "position"]);
  res[".project-arrow @375 (extends .row-arrow)"] = await cs(".project-arrow", ["position", "top", "right", "font-size", "opacity", "margin-left", "display"]);
  await go("/projects/", 1280);
  res[".project @1280 (scoped media rule vs .row)"] = await cs("a.project", ["align-items", "gap", "padding-top"]);
  await go("/blog/", 1280);
  res[".list-row-arrow (extends .row-arrow)"] = await cs(".list-row-arrow", ["position", "top", "font-size", "opacity", "margin-left", "display", "color"]);
  await page.hover("a.list-row"); await page.waitForTimeout(400);
  res[".list-row-arrow, row hovered (.row-link:hover .row-arrow)"] = await cs(".list-row-arrow", ["opacity", "transform"]);
  const post = base.includes(String(sAstro.port)) ? "/blog/extending-c-stdlib/" : "/blog/extending_c_stdlib/";
  await go(post, 1280);
  const ink = await page.evaluate(() => { const d = document.createElement("i"); d.style.color = "var(--ink)"; document.body.appendChild(d); const c = getComputedStyle(d).color; d.remove(); return c; });
  res[".meta-link.text-ink-2 at rest"] = await cs("p > a.meta-link", ["color"]);
  await page.hover("p > a.meta-link"); await page.waitForTimeout(400);
  res[".meta-link.text-ink-2 hovered (scoped :hover vs .text-ink-2)"] = { ...(await cs("p > a.meta-link", ["color"])), "is --ink": (await cs("p > a.meta-link", ["color"]))!.color === ink };
  await page.mouse.move(0, 0);
  return res;
}
const ref = await probe(`http://localhost:${sAstro.port}`, false);
const mine = await probe(`http://localhost:${sOurs.port}`, false);
const mineRev = await probe(`http://localhost:${sOurs.port}`, true);
const ctl = await probe(`http://localhost:${sOurs.port}`, true, true);
log("| Probe | Astro build | Ours (global CSS, then scoped) | Ours (scoped, then global CSS) | Control: same sidecars with the attribute removed, then global CSS |\n|---|---|---|---|---|");
for (const k of Object.keys(ref)) {
  const j = (x: any) => JSON.stringify(x).replace(/"/g, "");
  log(`| ${k} | ${j(ref[k])} | ${j(mine[k]) === j(ref[k]) ? "same" : "**" + j(mine[k]) + "**"} | ${j(mineRev[k]) === j(ref[k]) ? "same" : "**" + j(mineRev[k]) + "**"} | ${j(ctl[k]) === j(ref[k]) ? "same" : "**" + j(ctl[k]) + "**"} |`);
  if (JSON.stringify(mine[k]) !== JSON.stringify(ref[k])) failures++;
}
ok(Object.keys(ref).every((k) => JSON.stringify(ref[k]) === JSON.stringify(mine[k])), "(c) all probes resolve as in the Astro build with Astro's stylesheet order");
ok(Object.keys(ref).every((k) => JSON.stringify(ref[k]) === JSON.stringify(mineRev[k])), "(c) and still do with the scoped sheet loaded BEFORE the global one (the +0-1-0 carries them)");

log(`\n${failures} failures. Chromium ${browser.version()}.`);
writeFileSync(join(root, "results/verify.md"), out.join("\n") + "\n");
await browser.close(); sOurs.stop(); sAstro.stop();
process.exit(failures ? 1 : 0);
