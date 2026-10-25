// The Router's feel checks (#61): the prototype's (#26) and the Script
// lifecycle's (ADR 0013), on the fixture Site in tests/sites/router, built
// by the yuanme binary once per `[router] mode` and served as Netlify
// serves a Site.
//
//   node feel.ts --yuanme ../../../../target/debug/yuanme [--engines chromium,firefox,webkit]
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { createServer } from 'node:http';
import type { AddressInfo } from 'node:net';
import { tmpdir } from 'node:os';
import { extname, join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import { chromium, firefox, webkit, type Browser, type BrowserContextOptions, type Page } from 'playwright';

const { values: args } = parseArgs({
	options: {
		yuanme: { type: 'string', default: join(import.meta.dirname, '../../../../target/debug/yuanme') },
		engines: { type: 'string', default: 'chromium,firefox,webkit' },
	},
});
const FIXTURE = join(import.meta.dirname, '../sites/router');
const MODES = ['swap', 'off', 'native'] as const;
type Mode = (typeof MODES)[number];

/** The fixture built with `mode`, into a fresh folder. */
function build(mode: Mode, tmp: string): string {
	const project = join(tmp, mode);
	cpSync(FIXTURE, project, { recursive: true });
	const config = join(project, 'yuanme.toml');
	writeFileSync(config, readFileSync(config, 'utf8').replace('mode = "swap"', `mode = "${mode}"`));
	execFileSync(resolve(args.yuanme), ['build', '--strict'], { cwd: project, stdio: 'inherit' });
	return join(project, 'dist');
}

const TYPES: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.xml': 'application/xml',
};

type Server = { origin: string; requests: string[]; close: () => void };

/** Serves `root` as Netlify does: `/x` redirects to `/x/`, and a missing file gets 404.html. */
async function serve(root: string): Promise<Server> {
	const requests: string[] = [];
	const server = createServer((req, res) => {
		const url = new URL(req.url ?? '/', 'http://localhost');
		requests.push(url.pathname);
		let file = join(root, decodeURIComponent(url.pathname));
		if (existsSync(file) && statSync(file).isDirectory()) {
			if (!url.pathname.endsWith('/')) return void res.writeHead(301, { location: url.pathname + '/' }).end();
			file = join(file, 'index.html');
		}
		const status = existsSync(file) ? 200 : 404;
		if (status === 404) file = join(root, '404.html');
		res.writeHead(status, { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream', 'cache-control': 'no-store' });
		res.end(readFileSync(file));
	});
	await new Promise<void>((done) => server.listen(0, '127.0.0.1', done));
	const { port } = server.address() as AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, requests, close: () => server.close() };
}

const w = (page: Page) => page.evaluate(() => {
	const w = window as any;
	const canvas = document.querySelector('[data-film-canvas]') as any;
	return {
		path: location.pathname,
		title: document.title,
		page: document.documentElement.dataset.page,
		current: [...document.querySelectorAll('[data-nav-link][aria-current]')].map((a) => (a as HTMLElement).dataset.section),
		canvas: canvas?.__probeId as number,
		contexts: Number(sessionStorage.getItem('__ctx')),
		contents: document.querySelectorAll('[data-panel-content]').length,
		pres: document.querySelectorAll('main pre').length,
		copies: document.querySelectorAll('main pre > button.copy').length,
		counter: document.querySelector('[data-counter]')?.textContent ?? null,
		announcers: document.querySelectorAll('[aria-live]').length,
		announced: document.querySelector('[aria-live]')?.textContent ?? null,
		top: w.__top as number,
		inline: (w.__inline ?? 0) as number,
		life: [...(w.__life ?? [])] as string[],
		events: [...(w.__events ?? [])] as string[],
		navErrors: w.__navErrors as number,
		marker: w.__marker as number | undefined,
		active: document.activeElement === document.body ? 'body' : (document.activeElement as HTMLElement)?.dataset.section ?? document.activeElement?.tagName,
		scrollY: Math.round(scrollY),
		entries: (window as any).navigation?.entries().length ?? history.length,
	};
});

/** Does `act`, which must move to another Page in place, and waits for its `yuanme:page-load` and announcement. */
async function soft(page: Page, act: () => Promise<unknown>) {
	await page.evaluate(() => ((window as any).__marker = 1));
	const loads = await page.evaluate(() => (window as any).__events.filter((e: string) => e === 'page-load').length);
	await act();
	await page.waitForFunction((n) => (window as any).__events?.filter((e: string) => e === 'page-load').length > n, loads);
	await page.waitForTimeout(150);
	const s = await w(page);
	assert.equal(s.marker, 1, `${s.path} was a full load`);
	return s;
}

/** Does `act`, which must load another Page in full. */
async function full(page: Page, act: () => Promise<unknown>) {
	await page.evaluate(() => ((window as any).__marker = 1));
	await act();
	await page.waitForFunction(() => (window as any).__marker === undefined && document.readyState === 'complete');
	await page.waitForTimeout(100);
}

const click = (page: Page, selector: string) => () => page.click(selector);
const nav = (section: string) => `[data-nav-link][data-section=${section}]`;
const TITLES: Record<string, string> = { '/': 'Home', '/about/': 'About', '/projects/': 'Projects', '/posts/long/': 'Long' };
const SECTIONS: Record<string, string> = { '/': 'home', '/about/': 'about', '/projects/': 'projects', '/posts/long/': 'writing' };
const PAGES: Record<string, string> = { '/': 'home', '/about/': 'about', '/projects/': 'projects', '/posts/long/': 'post' };

type Open = (mode: Mode, options?: BrowserContextOptions, hideNavigation?: boolean) => Promise<{ page: Page; server: Server }>;
type Check = (open: Open) => Promise<void>;

const CHECKS: Record<string, Check> = {
	// The prototype's 12-step tour: the persisted canvas and nav survive,
	// each Page mounts its Sidecar scripts once and cleans up before the
	// next swap, and the swap brings the title, `<html>`'s attributes and
	// one announcer.
	async tour(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		const first = await w(page);
		assert.deepEqual(first.events, ['page-load']);
		assert.deepEqual(first.life, ['mount / 1']);
		assert.equal(first.counter, 'mounted 1');
		const tour: [string, () => Promise<unknown>][] = [
			['/about/', click(page, nav('about'))],
			['/posts/long/', click(page, nav('writing'))],
			['/projects/', click(page, nav('projects'))],
			['/posts/long/', () => page.goBack()],
			['/about/', () => page.goBack()],
			['/posts/long/', () => page.goForward()],
			['/posts/long/', click(page, nav('writing'))],
			['/', click(page, nav('home'))],
			['/posts/long/', click(page, 'main a[href="/posts/long/"]')],
			['/about/', click(page, nav('about'))],
			['/projects/', click(page, nav('projects'))],
			['/about/', click(page, nav('about'))],
		];
		const life = [...first.life];
		let mounts = 1;
		let counters = 1;
		let prev = '/';
		for (const [i, [path, act]] of tour.entries()) {
			const s = await soft(page, act);
			const at = `step ${i + 1} (${path})`;
			assert.equal(s.path, path, at);
			assert.equal(s.title, `${TITLES[path]} | Router`, at);
			assert.equal(s.page, PAGES[path], `${at}: <html>'s attributes`);
			assert.deepEqual(s.current, [SECTIONS[path]], `${at}: aria-current`);
			assert.equal(s.canvas, first.canvas, `${at}: the persisted canvas`);
			assert.equal(s.contexts, 1, `${at}: canvas contexts`);
			assert.equal(s.contents, 1, at);
			assert.equal(s.copies, s.pres, `${at}: a copy button on each code block`);
			assert.equal(s.top, 1, `${at}: a Sidecar script's top level runs once per document`);
			assert.equal(s.announcers, 1, at);
			assert.equal(s.announced, s.title, at);
			life.push(`abort ${prev}`, `cleanup ${prev}`, `mount ${path} ${++mounts}`);
			assert.deepEqual(s.life, life, `${at}: mount and cleanup`);
			if (path === '/' || path === '/projects/') assert.equal(s.counter, `mounted ${++counters}`, `${at}: ctx.data lasts across swaps`);
			assert.deepEqual(s.events.slice(-3), ['before-swap', 'after-swap', 'page-load'], at);
			assert.equal(s.events.length, 1 + 3 * (i + 1), at);
			prev = path;
		}
		const end = await w(page);
		assert.equal(end.inline, 1, 'an inline script runs once per document');
		assert.equal(end.navErrors, 0, 'navigateerror');
	},

	// Push and replace go to the top, traversals restore (Firefox 157's
	// scroll() left a push where it was).
	async scroll(open) {
		const { page, server } = await open('swap', { viewport: { width: 390, height: 844 } });
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await page.evaluate(() => scrollTo(0, 300));
		await page.waitForTimeout(100);
		const pushed = await soft(page, () => page.evaluate(() => (document.querySelector('[data-nav-link][data-section=about]') as HTMLElement).click()));
		assert.equal(pushed.scrollY, 0, 'push');
		const back = await soft(page, () => page.goBack());
		assert.equal(back.scrollY, 300, 'back');
		const forward = await soft(page, () => page.goForward());
		assert.equal(forward.scrollY, 0, 'forward');
		await page.evaluate(() => scrollTo(0, 200));
		const replaced = await soft(page, () => page.evaluate(() => (window as any).__navigate('/projects/', { history: 'replace' })));
		assert.equal(replaced.path, '/projects/');
		assert.equal(replaced.scrollY, 0, 'replace');
		assert.equal(replaced.entries, forward.entries, 'navigate() with history: "replace" adds no entry');
	},

	async fragment(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		const s = await soft(page, click(page, '#fragment'));
		assert.equal(s.path, '/posts/long/');
		const top = await page.evaluate(() => document.getElementById('section-2')!.getBoundingClientRect().top);
		assert.ok(s.scrollY > 0 && Math.abs(top) < 2, `section 2 is ${top}px from the top`);
	},

	// Focus stays on a keyboard-activated link that the swap keeps.
	async focus(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await page.focus(nav('about'));
		const s = await soft(page, () => page.keyboard.press('Enter'));
		assert.equal(s.path, '/about/');
		assert.equal(s.active, 'about');
	},

	// Two clicks 5 ms apart land on the second Page. The first may swap in
	// before the second click, as in Astro, but never after it.
	async abort(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await page.evaluate(async () => {
			(document.querySelector('[data-nav-link][data-section=about]') as HTMLElement).click();
			await new Promise((r) => setTimeout(r, 5));
			(document.querySelector('[data-nav-link][data-section=projects]') as HTMLElement).click();
		});
		await page.waitForTimeout(1000);
		const s = await w(page);
		assert.equal(s.path, '/projects/');
		assert.equal(s.title, 'Projects | Router');
		assert.equal(s.contents, 1);
		assert.match(s.life.at(-1)!, /^mount \/projects\/ /);
	},

	// What the Router leaves to the browser.
	async fullLoads(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await full(page, click(page, '#reload'));
		assert.equal((await w(page)).path, '/projects/', 'data-yuanme-reload');
		await full(page, click(page, '#plain'));
		assert.equal(await page.textContent('h1'), "A Static file the Router can't swap");
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await page.evaluate(() => ((window as any).__marker = 1));
		await page.click('#feed');
		await page.waitForURL('**/feed.xml');
		assert.equal(await page.evaluate(() => (window as any).__marker), undefined, 'a .xml link');
	},

	// /about redirects to /about/: the Router adopts the final URL with one
	// new entry, and back still works.
	async redirect(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		const before = await w(page);
		await page.evaluate(() => document.querySelector('main')!.insertAdjacentHTML('beforeend', '<a id="old" href="/about">Old</a>'));
		const s = await soft(page, click(page, '#old'));
		assert.equal(s.path, '/about/');
		assert.equal(s.title, 'About | Router');
		assert.equal(s.entries, before.entries + 1);
		assert.equal(s.navErrors, 0, 'navigateerror');
		const back = await soft(page, () => page.goBack());
		assert.equal(back.path, '/');
	},

	// Hover fetches the Page once, and the click that follows uses it.
	async prefetch(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		server.requests.length = 0;
		await page.hover(nav('about'));
		await page.waitForTimeout(300);
		assert.deepEqual(server.requests.filter((r) => r === '/about/'), ['/about/'], 'hover');
		await soft(page, click(page, nav('about')));
		assert.deepEqual(server.requests.filter((r) => r === '/about/'), ['/about/'], 'the click after it');
		await page.hover('#no-prefetch');
		await page.waitForTimeout(300);
		assert.deepEqual(server.requests.filter((r) => r === '/projects/'), [], 'data-yuanme-prefetch="false"');
	},

	// A Site may replace the swap from yuanme:before-swap.
	async customSwap(open) {
		const { page, server } = await open('swap');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await page.evaluate(() =>
			document.addEventListener('yuanme:before-swap', (e: any) => {
				const swap = e.swap;
				e.swap = () => {
					(window as any).__custom = e.newDocument.title;
					swap();
				};
			}, { once: true }),
		);
		const s = await soft(page, click(page, nav('about')));
		assert.equal(s.path, '/about/');
		assert.equal(await page.evaluate(() => (window as any).__custom), 'About | Router');
	},

	// Without the Navigation API, swap mode is full page loads that still
	// mount and fire yuanme:page-load once each.
	async noNavigationApi(open) {
		const { page, server } = await open('swap', {}, true);
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__events?.includes('page-load'));
		await full(page, click(page, nav('about')));
		const s = await w(page);
		assert.equal(s.path, '/about/');
		assert.deepEqual(s.life, ['mount /about/ 1']);
		assert.deepEqual(s.events, ['page-load']);
	},

	async off(open) {
		const { page, server } = await open('off');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__life?.length);
		assert.equal((await w(page)).counter, 'mounted 1');
		await full(page, click(page, nav('about')));
		const s = await w(page);
		assert.deepEqual(s.life, ['mount /about/ 1']);
		assert.deepEqual(s.events, []);
		assert.equal(s.copies, 2);
		assert.equal(await page.locator('script[data-yuanme-router]').count(), 0);
	},

	async native(open) {
		const { page, server } = await open('native');
		await page.goto(server.origin + '/');
		await page.waitForFunction(() => (window as any).__life?.length);
		const css = await page.evaluate(() => [...document.querySelectorAll('style')].map((s) => s.textContent).join(''));
		assert.match(css, /@view-transition\s*\{\s*navigation:\s*auto;?\s*\}/);
		await full(page, click(page, nav('about')));
		const s = await w(page);
		assert.deepEqual(s.life, ['mount /about/ 1']);
		assert.deepEqual(s.events, []);
	},
};

const ENGINES = { chromium, firefox, webkit };
const tmp = mkdtempSync(join(tmpdir(), 'yuanme-router-'));
let failed = 0;
try {
	const servers = Object.fromEntries(await Promise.all(MODES.map(async (m) => [m, await serve(build(m, tmp))]))) as Record<Mode, Server>;
	for (const name of args.engines.split(',')) {
		const browser: Browser = await ENGINES[name as keyof typeof ENGINES].launch();
		for (const [check, run] of Object.entries(CHECKS)) {
			const contexts: { close: () => Promise<void> }[] = [];
			try {
				await run(async (mode, options = {}, hideNavigation = false) => {
					const ctx = await browser.newContext({ viewport: { width: 1280, height: 800 }, ...options });
					contexts.push(ctx);
					ctx.setDefaultTimeout(5000);
					if (hideNavigation) await ctx.addInitScript('Object.defineProperty(window, "navigation", { value: undefined, configurable: true })');
					return { page: await ctx.newPage(), server: servers[mode] };
				});
				console.log(`ok   ${name} ${check}`);
			} catch (e) {
				failed++;
				console.log(`FAIL ${name} ${check}: ${(e as Error).message}`);
			} finally {
				await Promise.all(contexts.map((c) => c.close()));
			}
		}
		await browser.close();
	}
	for (const s of Object.values(servers)) s.close();
} finally {
	rmSync(tmp, { recursive: true, force: true });
}
if (failed) {
	console.log(`${failed} failed`);
	process.exit(1);
}
