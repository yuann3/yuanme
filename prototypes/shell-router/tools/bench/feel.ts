// Feel checks, Astro build vs prototype, same probe. Usage: bun bench/feel.ts <chromium|webkit> [ports=4321,4322,4324]
import { chromium, firefox, webkit, type Page } from 'playwright';
import { mkdirSync, writeFileSync } from 'node:fs';

const [bname = 'chromium', portArg = '4321,4322,4324'] = process.argv.slice(2);
const NAMES: Record<string, string> = { 4321: 'astro', 4322: 'proto', 4323: 'full', 4324: 'hist' };
const browser = await (bname === 'webkit' ? webkit : bname === 'firefox' ? firefox : chromium).launch();
const results: Record<string, Record<string, unknown>> = {};

async function open(port: number, opts: Parameters<typeof browser.newContext>[0] = {}) {
	const ctx = await browser.newContext({ viewport: { width: 1440, height: 900 }, ...opts });
	await ctx.addInitScript({ path: import.meta.dir + '/probe.js' });
	const page = await ctx.newPage();
	return { ctx, page, base: `http://${process.env.HOST ?? 'localhost'}:${port}` };
}
const settle = async (page: Page, ms = 1300) => page.waitForTimeout(ms);
const state = (page: Page) => page.evaluate(() => {
	const P = (window as any).__probe;
	const c = document.querySelector('[data-film-canvas]') as any;
	return {
		path: location.pathname, title: document.title,
		html: [...document.documentElement.attributes].map((a) => `${a.name}=${a.value}`).join(' '),
		current: [...document.querySelectorAll('[data-nav-link]')].map((a) => `${(a as HTMLElement).dataset.section}:${a.getAttribute('aria-current') ?? '-'}`).join(' '),
		canvasId: c?.__probeId ?? null, glCalls: P.gl.calls, ctxTab: +(sessionStorage.getItem('__ctx') || 0), lost: P.gl.lost + (P.gl.ctx?.isContextLost() ? 1 : 0),
		contents: document.querySelectorAll('[data-panel-content]').length,
		announcers: document.querySelectorAll('.astro-route-announcer').length,
		listeners: P.listeners.join(','),
		scripts: document.scripts.length,
		active: document.activeElement === document.body ? 'body' : `${document.activeElement?.tagName}.${(document.activeElement as HTMLElement)?.dataset?.section ?? (document.activeElement as HTMLElement)?.className}`,
		scrollY: Math.round(scrollY), panelTop: Math.round((document.querySelector('[data-panel-content]') as HTMLElement)?.scrollTop ?? -1),
		entries: (window as any).navigation?.entries?.().length ?? history.length,
	};
});

for (const port of portArg.split(',').map(Number)) {
	const name = NAMES[port];
	const R: Record<string, unknown> = (results[name] = {});

	// 1. Film continuity over a 10-step tour; aria-current, titles, announcers, script binding.
	{
		const { ctx, page, base } = await open(port);
		await page.goto(base + '/');
		await settle(page);
		const tour: [string, () => Promise<unknown>][] = [
			['click Writing', () => page.click('[data-nav-link][data-section=writing]')],
			['click post', () => page.click('[data-panel-content] a[href*="extending-c-stdlib"]')],
			['click Projects', () => page.click('[data-nav-link][data-section=projects]')],
			['back', () => page.evaluate(() => history.back())],
			['back', () => page.evaluate(() => history.back())],
			['forward', () => page.evaluate(() => history.forward())],
			['click Writing (same section)', () => page.click('[data-nav-link][data-section=writing]')],
			['click post again', () => page.click('[data-panel-content] a[href*="extending-c-stdlib"]')],
			['Esc', () => page.keyboard.press('Escape')],
			['click About', () => page.click('[data-nav-link][data-section=about]')],
			['click Resume', () => page.click('[data-nav-link][data-section=resume]')],
			['click About again', () => page.click('[data-nav-link][data-section=about]')],
		];
		const log: unknown[] = [];
		const first = await state(page);
		for (const [label, act] of tour) {
			await act();
			await settle(page);
			const s = await state(page);
			const extra = await page.evaluate(() => ({
				pres: document.querySelectorAll('[data-panel-content] pre').length,
				copyBlocks: document.querySelectorAll('[data-panel-content] [data-code-block]').length,
				copyButtons: document.querySelectorAll('[data-panel-content] [data-code-block] > button').length,
				piles: document.querySelectorAll('[data-panel-content] [data-pile]').length,
				pilesInit: document.querySelectorAll('[data-panel-content] [data-pile][data-init]').length,
			}));
			log.push({ label, ...s, ...extra });
		}
		const all = log as any[];
		R.tour = all.map((s) => `${s.label} -> ${s.path} [${s.current}] contents=${s.contents} ann=${s.announcers}`);
		R.filmSameCanvas = all.every((s) => s.canvasId === first.canvasId);
		R.filmContexts = Math.max(...all.map((s) => s.ctxTab));
		R.filmLost = all.reduce((a, s) => a + s.lost, 0);
		R.copyButtonsPerPre = all.filter((s) => s.pres).map((s) => `${s.copyButtons}/${s.pres}`);
		R.photoPileInit = all.filter((s) => s.piles).map((s) => `${s.pilesInit}/${s.piles}`);
		R.pageLoadListeners = all[all.length - 1].listeners;
		R.announcersAfterTour = all[all.length - 1].announcers;
		R.titles = all.map((s) => s.title.split(' | ')[0]);
		R.scriptsCount = all.map((s) => s.scripts).join(',');
		R.entries = all[all.length - 1].entries;
		await ctx.close();
	}

	// 2. Window scroll on mobile (the idle page scrolls the window); panel scroll on desktop back.
	{
		const { ctx, page, base } = await open(port, { viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
		await page.goto(base + '/');
		await settle(page);
		const maxY = await page.evaluate(() => document.documentElement.scrollHeight - innerHeight);
		await page.evaluate(() => scrollTo(0, 300));
		await page.waitForTimeout(300);
		const before = await page.evaluate(() => scrollY);
		await page.evaluate(() => (document.querySelector('[data-nav-link][data-section=writing]') as HTMLElement).click());
		await settle(page);
		const afterPush = await page.evaluate(() => scrollY);
		await page.evaluate(() => history.back());
		await settle(page);
		const afterBack = await page.evaluate(() => scrollY);
		await page.evaluate(() => history.forward());
		await settle(page);
		const afterFwd = await page.evaluate(() => scrollY);
		R.mobileWindowScroll = { maxY, before, afterPush, afterBack, afterFwd };
		await ctx.close();
	}
	{
		const { ctx, page, base } = await open(port);
		await page.goto(base + '/blog/extending-c-stdlib/');
		await settle(page);
		await page.evaluate(() => ((document.querySelector('[data-panel-content]') as HTMLElement).scrollTop = 2000));
		await page.waitForTimeout(200);
		await page.click('[data-nav-link][data-section=projects]');
		await settle(page);
		await page.evaluate(() => history.back());
		await settle(page);
		R.desktopPanelScrollAfterBack = (await state(page)).panelTop;
		await ctx.close();
	}

	// 3. Reduced motion.
	{
		const { ctx, page, base } = await open(port, { reducedMotion: 'reduce' });
		await page.goto(base + '/');
		await settle(page);
		await page.click('[data-nav-link][data-section=writing]');
		const rm = await page.waitForFunction(() => document.querySelector('[data-panel-content] .panel-title') && location.pathname.startsWith('/blog'), null, { timeout: 5000 }).then(() => page.evaluate(() => {
			const panel = document.querySelector('[data-panel]') as HTMLElement;
			const items = [...document.querySelectorAll('[data-panel-content] [data-rv]')] as HTMLElement[];
			return {
				morphAnims: panel.getAnimations().length + (document.querySelector('[data-film-layer]') as HTMLElement).getAnimations().length,
				rvBases: [...new Set(items.map((i) => i.style.getPropertyValue('--rv-base') || '(unset)'))].join(','),
				filmReady: panel.dataset.film ?? '(unset)',
				glCalls: (window as any).__probe.gl.calls,
			};
		}));
		await settle(page);
		const t = await page.evaluate(() => performance.now());
		await page.keyboard.press('Escape');
		await page.waitForFunction(() => location.pathname === '/', null, { timeout: 3000 });
		const escMs = (await page.evaluate(() => performance.now())) - t;
		R.reducedMotion = { ...rm, escToUrlMs: Math.round(escMs) };
		await ctx.close();
	}

	// 4. Focus: keyboard-activate a rail link; activate a link inside the panel.
	{
		const { ctx, page, base } = await open(port);
		await page.goto(base + '/');
		await settle(page);
		await page.focus('[data-nav-link][data-section=writing]');
		await page.keyboard.press('Enter');
		await settle(page);
		const afterRail = (await state(page)).active;
		await page.focus('[data-panel-content] a[href*="extending-c-stdlib"]');
		await page.keyboard.press('Enter');
		await settle(page);
		const afterPanel = (await state(page)).active;
		await page.keyboard.press('Escape');
		await settle(page);
		const afterEsc = (await state(page)).active;
		R.focus = { afterRail, afterPanel, afterEsc };
		await ctx.close();
	}

	// 5. Abort: two clicks 5 ms apart; redirect adoption; rss full load.
	{
		const { ctx, page, base } = await open(port);
		await page.goto(base + '/');
		await settle(page);
		await page.evaluate(async () => {
			(document.querySelector('[data-nav-link][data-section=writing]') as HTMLElement).click();
			await new Promise((r) => setTimeout(r, 5));
			(document.querySelector('[data-nav-link][data-section=projects]') as HTMLElement).click();
		});
		await settle(page, 1800);
		const s = await state(page);
		const swaps = await page.evaluate(() => (window as any).__probe.events.filter((e: any) => e[0] === 'after-swap').length);
		R.abort = { path: s.path, title: s.title.split(' | ')[0], contents: s.contents, afterSwaps: swaps, entries: s.entries };
		await page.click('a[href="/rss.xml"]');
		await page.waitForTimeout(800);
		R.rss = page.url().endsWith('/rss.xml') ? 'full load' : page.url();
		await ctx.close();
	}
	console.log(name, JSON.stringify(R, null, 1));
}
const version = browser.version();
await browser.close();
mkdirSync(import.meta.dir + '/../../results', { recursive: true });
writeFileSync(import.meta.dir + `/../../results/feel-${bname}.json`, JSON.stringify({ browser: bname, version, results }, null, 1));
