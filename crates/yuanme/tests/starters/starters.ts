// The Starters' browser checks (#36, #70): each Starter, made with
// `yuanme init` and built with `--strict-budgets`, has no axe-core
// violation on any Page, in light and dark, and the blog's copy button and
// skip link work. With --lab, the release's check instead: mobile LCP
// ≤ 2.0 s and CLS ≤ 0.01 on each Starter's home Page and the blog's Post.
//
//   node starters.ts --yuanme ../../../../target/debug/yuanme [--lab] [--shots DIR]
//
// --shots writes a screenshot of each Page in each scheme, to look at.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { extname, join, relative, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { AxeBuilder } from '@axe-core/playwright';
import { chromium, devices, type Browser, type Page } from 'playwright';

const { values: args } = parseArgs({
	options: {
		yuanme: { type: 'string', default: join(import.meta.dirname, '../../../../target/debug/yuanme') },
		lab: { type: 'boolean', default: false },
		shots: { type: 'string' },
	},
});
const STARTERS = ['blog', 'minimal'] as const;
type Starter = (typeof STARTERS)[number];

/** `starter`, made by `yuanme init` and built with `--strict-budgets`. */
function build(starter: Starter, tmp: string): string {
	const yuanme = resolve(args.yuanme);
	execFileSync(yuanme, ['init', starter, '--starter', starter, '-q'], { cwd: tmp, stdio: 'inherit' });
	const project = join(tmp, starter);
	execFileSync(yuanme, ['build', '--strict-budgets', '-q'], { cwd: project, stdio: 'inherit' });
	return join(project, 'dist');
}

const TYPES: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.svg': 'image/svg+xml',
	'.woff2': 'font/woff2',
	'.xml': 'application/xml',
};

/** Serves `root` as a static host does: `/x` redirects to `/x/`, and a missing file gets 404.html. */
async function serve(root: string) {
	const server = createServer((req, res) => {
		const url = new URL(req.url ?? '/', 'http://localhost');
		let file = join(root, decodeURIComponent(url.pathname));
		if (existsSync(file) && statSync(file).isDirectory()) {
			if (!url.pathname.endsWith('/')) return void res.writeHead(301, { location: url.pathname + '/' }).end();
			file = join(file, 'index.html');
		}
		const status = existsSync(file) ? 200 : 404;
		if (status === 404) file = join(root, '404.html');
		res.writeHead(status, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' });
		res.end(readFileSync(file));
	});
	await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
	const { port } = server.address() as AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, close: () => server.close() };
}

/** Every HTML Page of `dist`, by its URL. */
function pages(dist: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		for (const name of readdirSync(dir)) {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) walk(path);
			else if (name.endsWith('.html')) out.push('/' + relative(dist, path).replaceAll('\\', '/').replace(/(^|\/)index\.html$/, '$1'));
		}
	};
	walk(dist);
	return out.sort();
}

const SCHEMES = ['light', 'dark'] as const;

async function accessibility(browser: Browser, starter: Starter, origin: string, urls: string[]) {
	const failures: string[] = [];
	for (const colorScheme of SCHEMES) {
		const context = await browser.newContext({ colorScheme });
		for (const url of urls) {
			const page = await context.newPage();
			await page.goto(origin + url, { waitUntil: 'load' });
			const { violations } = await new AxeBuilder({ page })
				.withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa', 'best-practice'])
				.analyze();
			for (const v of violations) {
				const nodes = v.nodes.map((n) => `    ${n.target.join(' ')}: ${n.failureSummary?.replaceAll('\n', ' ')}`);
				failures.push(`${starter} ${url} (${colorScheme}): ${v.id}, ${v.help}\n${nodes.join('\n')}`);
			}
			if (args.shots) {
				mkdirSync(args.shots, { recursive: true });
				const name = `${starter}${url.replaceAll('/', '_').replace(/_$/, '')}-${colorScheme}.png`;
				await page.screenshot({ path: join(args.shots, name), fullPage: true });
			}
			await page.close();
		}
		await context.close();
	}
	assert.deepEqual(failures, [], `axe-core found violations:\n${failures.join('\n')}`);
	console.log(`${starter}: ${urls.length} Pages, no axe-core violation in light or dark`);
}

/** The blog's own behaviour: its skip link is the first stop, and each code block's button copies its code. */
async function blog(browser: Browser, origin: string) {
	const context = await browser.newContext({ permissions: ['clipboard-read', 'clipboard-write'] });
	const page = await context.newPage();
	await page.goto(origin + '/blog/hello/');
	await page.keyboard.press('Tab');
	assert.equal(await page.evaluate(() => document.activeElement?.textContent), 'Skip to content');
	await page.keyboard.press('Enter');
	assert.equal(new URL(page.url()).hash, '#main');

	const blocks = page.locator('pre.y-code');
	assert.equal(await page.locator('.code-copy').count(), await blocks.count());
	const button = page.locator('.code-copy').first();
	await button.click();
	const code = await blocks.first().locator('code').textContent();
	assert.equal(await page.evaluate(() => navigator.clipboard.readText()), code);
	assert.equal(await button.textContent(), 'Copied');
	assert.equal(await page.locator('[role=status]').textContent(), 'Copied');
	await context.close();
	console.log('blog: the skip link and the copy button work');
}

/**
 * Mobile LCP and CLS of `url`, in a phone's viewport on Lighthouse's mobile
 * throttling as DevTools applies it: 4x slower CPU, 562.5 ms round trips
 * and 1.6 Mbps down.
 */
async function lab(browser: Browser, url: string) {
	const context = await browser.newContext({ ...devices['Moto G Power'] });
	const page = await context.newPage();
	const cdp = await context.newCDPSession(page);
	await cdp.send('Emulation.setCPUThrottlingRate', { rate: 4 });
	await cdp.send('Network.enable');
	await cdp.send('Network.emulateNetworkConditions', {
		offline: false,
		latency: 562.5,
		downloadThroughput: (1638.4 * 1024 * 0.9) / 8,
		uploadThroughput: (750 * 1024 * 0.9) / 8,
	});
	await page.addInitScript(() => {
		const w = window as any;
		w.__lcp = 0;
		w.__cls = 0;
		new PerformanceObserver((list) => {
			for (const e of list.getEntries()) w.__lcp = e.startTime;
		}).observe({ type: 'largest-contentful-paint', buffered: true });
		new PerformanceObserver((list) => {
			for (const e of list.getEntries() as any[]) if (!e.hadRecentInput) w.__cls += e.value;
		}).observe({ type: 'layout-shift', buffered: true });
	});
	await page.goto(url, { waitUntil: 'networkidle' });
	await page.waitForTimeout(1000);
	const result = await page.evaluate(() => ({ lcp: (window as any).__lcp as number, cls: (window as any).__cls as number }));
	await context.close();
	return result;
}

const tmp = mkdtempSync(join(tmpdir(), 'yuanme-starters-'));
const browser = await chromium.launch();
try {
	for (const starter of STARTERS) {
		const dist = build(starter, tmp);
		const server = await serve(dist);
		try {
			if (args.lab) {
				const urls = starter === 'blog' ? ['/', '/blog/hello/'] : ['/'];
				for (const url of urls) {
					const { lcp, cls } = await lab(browser, server.origin + url);
					console.log(`${starter} ${url}: LCP ${Math.round(lcp)} ms, CLS ${cls.toFixed(3)}`);
					assert.ok(lcp > 0 && lcp <= 2000, `${starter} ${url}: LCP ${lcp} ms, over 2.0 s`);
					assert.ok(cls <= 0.01, `${starter} ${url}: CLS ${cls}, over 0.01`);
				}
				continue;
			}
			await accessibility(browser, starter, server.origin, pages(dist));
			if (starter === 'blog') await blog(browser, server.origin);
		} finally {
			server.close();
		}
	}
} finally {
	await browser.close();
}
