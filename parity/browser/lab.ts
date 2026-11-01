// The Reference site's lab Gates (#15, #39): LCP, CLS and TBT of `/`, a
// Post and `/about/`, cold, on Lighthouse's mobile and desktop throttling
// as DevTools applies it, the median of RUNS loads of each, with #15's 10%
// tolerance. Exits 1 if one is over its Gate. Desktop LCP over 0.8 s, and
// the Post's mobile LCP over 2.0 s, are reported, not failed on, until the
// Site's first reveal allows them (#95).
//
// The mobile CPU slowdown is calibrated to this host, as Lighthouse's
// throttling docs say to: a fixed 4× is a mid-tier phone only on a
// high-end desktop. On a host whose WebGL is SwiftShader, as on CI, WebGL
// is off: the GPU process would draw the Film's frames on the CPU, against
// the Page's main thread, which a phone's GPU doesn't.
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
/** Whether a Page's LCP fails the run, or is a warning until #95. */
const gated = (preset: string, url: string) => preset === 'mobile' && url !== '/blog/extending-c-stdlib/';

type Preset = { name: string; options: BrowserContextOptions; cpu: number; latency: number; kbps: number; lcp: number };

/** #15's tolerance on every lab Gate. */
const TOLERANCE = 1.1;
const CLS = 0.01;
const TBT = 50;

/**
 * Lighthouse's BenchmarkIndex of this host's CPU: its computeBenchmarkIndex,
 * from core/lib/page-functions.js, the median of three.
 */
async function benchmarkIndex(browser: Browser): Promise<number> {
	const page = await browser.newPage();
	const runs = [];
	for (let i = 0; i < 3; i++) {
		runs.push(
			await page.evaluate(() => {
				const gc = () => {
					const start = Date.now();
					let iterations = 0;
					while (Date.now() - start < 500) {
						let s = '';
						for (let j = 0; j < 10000; j++) s += 'a';
						if (s.length === 1) throw new Error('never');
						iterations++;
					}
					return Math.round(iterations / 10 / ((Date.now() - start) / 1000));
				};
				const noGc = () => {
					const a: number[] = [];
					const b: number[] = [];
					for (let i = 0; i < 100000; i++) a[i] = b[i] = i;
					const start = Date.now();
					let iterations = 0;
					while (iterations % 10 !== 0 || Date.now() - start < 500) {
						const [src, tgt] = iterations % 2 === 0 ? [a, b] : [b, a];
						for (let j = 0; j < src.length; j++) tgt[j] = src[j];
						iterations++;
					}
					return Math.round(iterations / 10 / ((Date.now() - start) / 1000));
				};
				return (gc() + noGc()) / 2;
			}),
		);
	}
	await page.close();
	return median(runs);
}

/**
 * The CPU slowdown that makes a host of this BenchmarkIndex a mid-tier
 * phone: Lighthouse's CPU throttling calculator, which its throttling docs
 * link to.
 */
function slowdown(index: number): number {
	if (index >= 1300) return 3 + (index - 1300) / 233;
	if (index >= 800) return 2 + (index - 800) / 500;
	return Math.max(1, 1 + (index - 150) / 650);
}

/** Whether this browser draws WebGL in software, on the CPU. */
async function softwareGl(browser: Browser): Promise<boolean> {
	const page = await browser.newPage();
	const renderer = await page.evaluate(() => {
		const gl = document.createElement('canvas').getContext('webgl');
		const info = gl?.getExtension('WEBGL_debug_renderer_info');
		return gl && info ? String(gl.getParameter(info.UNMASKED_RENDERER_WEBGL)) : '';
	});
	await page.close();
	return /swiftshader/i.test(renderer);
}

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

const probe = await chromium.launch();
const index = await benchmarkIndex(probe);
const software = await softwareGl(probe);
await probe.close();
const cpu = Math.round(slowdown(index) * 10) / 10;
console.log(`BenchmarkIndex ${Math.round(index)}: mobile CPU slowdown ${cpu}×${software ? '; WebGL is SwiftShader here, so it is off' : ''}`);
/**
 * Lighthouse's presets: a Moto G Power's screen at 150 ms and 1.6 Mbps on a
 * mid-tier phone's CPU, and a desktop's at 40 ms and 10 Mbps. DevTools
 * multiplies the round trip by 3.75 and the throughput by 0.9. Playwright
 * has no Moto G Power, so its screen is Lighthouse's own numbers.
 */
const PRESETS: Preset[] = [
	{ name: 'mobile', options: { viewport: { width: 412, height: 823 }, deviceScaleFactor: 1.75, isMobile: true, hasTouch: true }, cpu, latency: 150 * 3.75, kbps: 1638.4 * 0.9, lcp: 2000 },
	{ name: 'desktop', options: { viewport: { width: 1350, height: 940 }, deviceScaleFactor: 1 }, cpu: 1, latency: 40 * 3.75, kbps: 10240 * 0.9, lcp: 800 },
];

const server = await serve(site, { host: true });
const browser = await chromium.launch(software ? { args: ['--disable-3d-apis'] } : {});
const failures: string[] = [];
try {
	for (const preset of PRESETS) {
		for (const url of URLS) {
			const runs: Result[] = [];
			for (let i = 0; i < Number(values.runs); i++) runs.push(await load(browser, preset, server.origin + url));
			const [lcp, cls, tbt] = (['lcp', 'cls', 'tbt'] as const).map((k) => median(runs.map((r) => r[k])));
			console.log(`${preset.name} ${url}: LCP ${Math.round(lcp)} ms, CLS ${cls.toFixed(3)}, TBT ${Math.round(tbt)} ms (median of ${runs.length}; LCP runs ${runs.map((r) => Math.round(r.lcp)).join(', ')})`);
			const slow = `${preset.name} ${url}: LCP ${Math.round(lcp)} ms, over ${preset.lcp} ms and its 10% tolerance`;
			if (!(lcp > 0)) failures.push(`${preset.name} ${url}: no LCP`);
			else if (lcp > preset.lcp * TOLERANCE && gated(preset.name, url)) failures.push(slow);
			else if (lcp > preset.lcp * TOLERANCE) console.log(`::warning::${slow} (#95)`);
			if (cls > CLS * TOLERANCE) failures.push(`${preset.name} ${url}: CLS ${cls.toFixed(3)}, over ${CLS} and its 10% tolerance`);
			if (tbt > TBT * TOLERANCE) failures.push(`${preset.name} ${url}: TBT ${Math.round(tbt)} ms, over ${TBT} ms and its 10% tolerance`);
		}
	}
} finally {
	await browser.close();
	await server.close();
}
failures.forEach((f) => console.log(f));
console.log(`lab Gates: ${failures.length} over`);
process.exit(failures.length ? 1 : 0);
