// The Reference site's lab Gates (#15, #39): LCP, CLS and TBT of `/`, a
// Post and `/about/`, cold, on Lighthouse's mobile and desktop throttling
// as DevTools applies it, the median of RUNS loads of each. Exits 1 if one
// is over its Gate. Desktop LCP over 0.8 s is reported, not failed on,
// until the Site's first reveal allows it (#95).
//
//   node lab.ts SITE [--runs 5]
import { parseArgs } from 'node:util';
import { chromium, type Browser, type BrowserContextOptions } from 'playwright';
import { serve } from './lib.ts';

const { positionals, values } = parseArgs({ allowPositionals: true, options: { runs: { type: 'string', default: '5' } } });
const [site] = positionals;
if (!site) {
	console.error('usage: node lab.ts SITE [--runs 5]');
	process.exit(2);
}

/** The Post with the largest image, a hero that was a 1.6 MB PNG (#13). */
const URLS = ['/', '/blog/extending-c-stdlib/', '/about/'];

type Preset = { name: string; options: BrowserContextOptions; cpu: number; latency: number; kbps: number; lcp: number; gated: boolean };
/**
 * Lighthouse's presets: a Moto G Power's screen at 150 ms and 1.6 Mbps on a
 * 4× slower CPU, and a desktop's at 40 ms and 10 Mbps. DevTools multiplies
 * the round trip by 3.75 and the throughput by 0.9. Playwright has no Moto
 * G Power, so its screen is Lighthouse's own numbers.
 */
const PRESETS: Preset[] = [
	{ name: 'mobile', options: { viewport: { width: 412, height: 823 }, deviceScaleFactor: 1.75, isMobile: true, hasTouch: true }, cpu: 4, latency: 150 * 3.75, kbps: 1638.4 * 0.9, lcp: 2000, gated: true },
	{ name: 'desktop', options: { viewport: { width: 1350, height: 940 }, deviceScaleFactor: 1 }, cpu: 1, latency: 40 * 3.75, kbps: 10240 * 0.9, lcp: 800, gated: false },
];
const CLS = 0.01;
const TBT = 50;

type Result = { lcp: number; cls: number; tbt: number };

/** One cold load of `url`. */
async function load(browser: Browser, preset: Preset, url: string): Promise<Result> {
	const context = await browser.newContext(preset.options);
	// Nothing but the Site: a request elsewhere would time the network.
	const origin = new URL(url).origin;
	await context.route('**/*', (route) => (route.request().url().startsWith(origin + '/') ? route.continue() : route.abort('blockedbyclient')));
	const page = await context.newPage();
	const cdp = await context.newCDPSession(page);
	await cdp.send('Emulation.setCPUThrottlingRate', { rate: preset.cpu });
	await cdp.send('Network.enable');
	await cdp.send('Network.emulateNetworkConditions', {
		offline: false,
		latency: preset.latency,
		downloadThroughput: (preset.kbps * 1024) / 8,
		uploadThroughput: (preset.kbps * 1024) / 8,
	});
	await page.addInitScript(() => {
		const w = window as any;
		w.__lab = { lcp: 0, cls: 0, fcp: Infinity, tasks: [] as [number, number][] };
		const observe = (type: string, f: (e: any) => void) => new PerformanceObserver((list) => list.getEntries().forEach(f)).observe({ type, buffered: true });
		observe('largest-contentful-paint', (e) => (w.__lab.lcp = e.startTime));
		observe('layout-shift', (e) => e.hadRecentInput || (w.__lab.cls += e.value));
		observe('paint', (e) => e.name === 'first-contentful-paint' && (w.__lab.fcp = e.startTime));
		observe('longtask', (e) => w.__lab.tasks.push([e.startTime, e.duration]));
	});
	await page.goto(url, { waitUntil: 'networkidle' });
	await page.waitForTimeout(1000);
	const result = await page.evaluate(() => {
		const { lcp, cls, fcp, tasks } = (window as any).__lab;
		// Blocking time after the first paint, as Lighthouse counts it.
		const tbt = (tasks as [number, number][]).filter(([at]) => at >= fcp).reduce((sum, [, d]) => sum + Math.max(0, d - 50), 0);
		return { lcp, cls, tbt };
	});
	await context.close();
	return result;
}

const median = (xs: number[]) => {
	const s = [...xs].sort((a, b) => a - b);
	return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};

const server = await serve(site, { host: true });
const browser = await chromium.launch();
const failures: string[] = [];
try {
	for (const preset of PRESETS) {
		for (const url of URLS) {
			const runs: Result[] = [];
			for (let i = 0; i < Number(values.runs); i++) runs.push(await load(browser, preset, server.origin + url));
			const [lcp, cls, tbt] = (['lcp', 'cls', 'tbt'] as const).map((k) => median(runs.map((r) => r[k])));
			console.log(`${preset.name} ${url}: LCP ${Math.round(lcp)} ms, CLS ${cls.toFixed(3)}, TBT ${Math.round(tbt)} ms (median of ${runs.length}; LCP runs ${runs.map((r) => Math.round(r.lcp)).join(', ')})`);
			const slow = `${preset.name} ${url}: LCP ${Math.round(lcp)} ms, over ${preset.lcp} ms`;
			if (!(lcp > 0)) failures.push(`${preset.name} ${url}: no LCP`);
			else if (lcp > preset.lcp && preset.gated) failures.push(slow);
			else if (lcp > preset.lcp) console.log(`::warning::${slow} (#95)`);
			if (cls > CLS) failures.push(`${preset.name} ${url}: CLS ${cls.toFixed(3)}, over ${CLS}`);
			if (tbt > TBT) failures.push(`${preset.name} ${url}: TBT ${Math.round(tbt)} ms, over ${TBT} ms`);
		}
	}
} finally {
	await browser.close();
	await server.close();
}
failures.forEach((f) => console.log(f));
console.log(`lab Gates: ${failures.length} over`);
process.exit(failures.length ? 1 : 0);
