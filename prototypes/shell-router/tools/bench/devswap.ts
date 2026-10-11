// Browser half of save-to-paint. Simulates the dev server's WebSocket push: the page
// already holds the new HTML (its own HTML with an edit), then either
//   soft  - hands it to the router's dev hook (yuanme:patch -> parse -> quiet shell swap)
//   reload - location.reload() (what a dev server without a morph does)
// Usage: bun bench/devswap.ts <chromium|webkit> <headless|headed|throttle4x> [runs=30]
import { chromium, firefox, webkit } from 'playwright';
import { mkdirSync, writeFileSync } from 'node:fs';
import { pct } from './stats';

const [bname = 'chromium', mode = 'headless', runsArg = '30'] = process.argv.slice(2);
const runs = +runsArg;
const browser = await (bname === 'webkit' ? webkit : bname === 'firefox' ? firefox : chromium).launch({ headless: mode !== 'headed' });
const pages = ['/blog/', '/blog/extending-c-stdlib/'];
const rows: any[] = [];

const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
await ctx.addInitScript({ path: import.meta.dir + '/probe.js' });
const page = await ctx.newPage();
if (mode === 'throttle4x') {
	const cdp = await ctx.newCDPSession(page);
	await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
}
const done = async () => {
	for (let i = 0; i < 400; i++) {
		try {
			const d = await page.evaluate(() => (window as any).__probe?.done);
			if (d) return d;
		} catch {}
		await page.waitForTimeout(25);
	}
	throw new Error('timeout');
};

for (const path of pages) {
	await page.goto(`http://${process.env.HOST ?? 'localhost'}:4322` + path);
	await done();
	for (let run = 0; run < runs; run++) {
		await page.waitForTimeout(400);
		// soft: scroll the panel, then push the edited HTML through the dev hook
		const soft = await page.evaluate(async (n) => {
			const html = (await (await fetch(location.href)).text()).replace(/(<h1 class="panel-title"[^>]*>)/, `$1EDIT${n} `);
			const panel = document.querySelector('[data-panel-content]') as HTMLElement;
			panel.scrollTop = 400;
			const canvas = document.querySelector('[data-film-canvas]');
			await new Promise((r) => requestAnimationFrame(r));
			const t0 = performance.now();
			document.dispatchEvent(new CustomEvent('yuanme:patch', { detail: html }));
			const swapped = performance.now();
			const painted: number = await new Promise((r) => requestAnimationFrame(() => {
				const mc = new MessageChannel();
				mc.port1.onmessage = () => r(performance.now());
				mc.port2.postMessage(0);
			}));
			const h1 = document.querySelector('[data-panel-content] .panel-title') as HTMLElement;
			return {
				swap: swapped - t0, painted: painted - t0,
				visible: h1.textContent!.includes(`EDIT${n}`) && getComputedStyle(h1).opacity === '1',
				sameCanvas: document.querySelector('[data-film-canvas]') === canvas,
				scrollKept: (document.querySelector('[data-panel-content]') as HTMLElement).scrollTop,
				htmlBytes: html.length,
			};
		}, run);
		rows.push({ path, kind: 'soft', run, ...soft });
		// reload
		await page.waitForTimeout(400);
		await page.evaluate(() => {
			(window as any).__probe.done = null;
			sessionStorage.setItem('__act', String(performance.timeOrigin + performance.now()));
			location.reload();
		});
		const r = await done();
		rows.push({ path, kind: 'reload', run, painted: r.painted, settled: r.settled, film: r.film?.firstFrame ?? null, glInit: r.film?.init ?? null });
	}
}
const version = browser.version();
await browser.close();
const out = import.meta.dir + '/../../results';
mkdirSync(out, { recursive: true });
writeFileSync(`${out}/devswap-${bname}-${mode}.json`, JSON.stringify({ browser: bname, version, mode, runs, rows }, null, 1));
const f = (xs: number[]) => `${Math.round(pct(xs, .5) * 10) / 10} / ${Math.round(pct(xs, .95) * 10) / 10}`;
console.log(`${bname} ${version} ${mode}`);
for (const path of pages) {
	const s = rows.filter((r) => r.path === path && r.kind === 'soft');
	const l = rows.filter((r) => r.path === path && r.kind === 'reload');
	console.log(`${path} soft: swap ${f(s.map((r) => r.swap))} painted ${f(s.map((r) => r.painted))} | visible ${s.every((r) => r.visible)} sameCanvas ${s.every((r) => r.sameCanvas)} scrollKept ${[...new Set(s.map((r) => r.scrollKept))]}`);
	console.log(`${path} reload: painted ${f(l.map((r) => r.painted))} film ${f(l.map((r) => r.film))} settled ${f(l.map((r) => r.settled))}`);
}
