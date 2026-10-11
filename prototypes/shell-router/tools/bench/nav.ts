// Navigation timing, soft swap vs full reload. Usage:
//   bun bench/nav.ts <chromium|webkit> <headless|headed> [runs=20] [variants=astro,proto,full,hist]
// Each run: cold "/" -> A idle→expanded (click Writing) -> B expanded→expanded (click Projects)
// -> C expanded→idle (click Close) -> back (idle→expanded) -> fwd (expanded→idle).
import { chromium, firefox, webkit } from 'playwright';
import { mkdirSync, writeFileSync } from 'node:fs';
import { summary, type Row } from './stats';

const [bname = 'chromium', mode = 'headless', runsArg = '20', varArg = 'astro,proto,full,hist'] = process.argv.slice(2);
const runs = +runsArg;
const PORTS: Record<string, number> = { astro: 4321, proto: 4322, full: 4323, hist: 4324, astronovt: 4321 };
const variants = varArg.split(',');
const bt = bname === 'webkit' ? webkit : bname === 'firefox' ? firefox : chromium;
const browser = await bt.launch({ headless: mode === 'headless' });
const rows: Row[] = [];
let renderer = '';

for (let run = 0; run < runs; run++) {
	for (const v of variants) {
		const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 } });
		await ctx.addInitScript({ path: import.meta.dir + '/probe.js' });
		if (v === 'astronovt') await ctx.addInitScript(() => { delete (Document.prototype as any).startViewTransition; });
		const page = await ctx.newPage();
		const base = `http://${process.env.HOST ?? 'localhost'}:${PORTS[v]}`;
		const done = async () => {
			for (let i = 0; i < 400; i++) {
				try {
					const d = await page.evaluate(() => {
						const P = (window as any).__probe;
						if (!P?.done) return null;
						const { act } = P;
						const end = act + P.done.settled - performance.timeOrigin;
						const start = act - performance.timeOrigin;
						const inWin = P.shifts.filter((s: any) => s[0] >= start - 1 && s[0] <= end + 1);
						return {
							...P.done,
							clsAll: inWin.reduce((a: number, s: any) => a + s[1], 0),
							cls: inWin.filter((s: any) => !s[2]).reduce((a: number, s: any) => a + s[1], 0),
							renderer: P.gl.renderer,
							ctxTab: +(sessionStorage.getItem('__ctx') || 0),
							lost: P.gl.ctx ? P.gl.ctx.isContextLost() : null,
						};
					});
					if (d) return d;
				} catch {}
				await page.waitForTimeout(25);
			}
			throw new Error('timeout');
		};
		const clear = () => page.evaluate(() => { (window as any).__probe.done = null; });
		const prog = (fn: string) => page.evaluate((f) => {
			(window as any).__probe.done = null;
			sessionStorage.setItem('__act', String(performance.timeOrigin + performance.now()));
			(history as any)[f]();
		}, fn);
		const steps: [string, () => Promise<unknown>][] = [
			['A idle→expanded', () => page.click('[data-nav-link][data-section=writing]')],
			['B expanded→expanded', () => page.click('[data-nav-link][data-section=projects]')],
			['C expanded→idle', () => page.click('[data-panel-content] a.close-desktop')],
			['back idle→expanded', () => prog('back')],
			['fwd expanded→idle', () => prog('forward')],
		];
		try {
			await page.goto(base + '/');
			const cold = await done();
			renderer ||= cold.renderer;
			rows.push({ browser: bname, mode, variant: v, step: 'cold /', run, ...cold });
			for (const [step, act] of steps) {
				await page.waitForTimeout(1200);
				if (!step.startsWith('back') && !step.startsWith('fwd')) await clear();
				await act();
				const r = await done();
				rows.push({ browser: bname, mode, variant: v, step, run, ...r });
			}
		} catch (e) {
			console.error(v, run, String(e).slice(0, 200));
		}
		await ctx.close();
	}
	process.stdout.write(`.${run}`);
}
const version = browser.version();
await browser.close();
const out = import.meta.dir + '/../../results';
mkdirSync(out, { recursive: true });
const file = `${out}/nav-${bname}-${mode}${varArg === "astro,proto,full,hist" ? "" : "-" + variants.join("_")}.json`;
writeFileSync(file, JSON.stringify({ browser: bname, version, mode, renderer, runs, rows }, null, 1));
console.log(`\n${bname} ${version} ${mode} renderer=${renderer}`);
console.log(summary(rows));
