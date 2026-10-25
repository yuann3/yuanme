// The behaviour check (#23): the prototype's feel.ts (#26), run on both
// sides in the same engine and compared. Its timings are left out: they are
// the Budgets' business, not parity's.
import { join } from 'node:path';
import type { Browser, BrowserContext, BrowserContextOptions, Page } from 'playwright';
import { context, type Server } from './lib.ts';

/** Results that differ on purpose, by the Parity exception that says so. */
export const TOLERATED: Record<string, string> = {
	announcers: 'route-announcer',
	desktopPanelScrollAfterBack: 'panel-scroll',
	scriptsCount: 'single-initial-script',
};
/** probe.js listens for both prefixes' events. */
export const RULES = [...Object.values(TOLERATED), 'router-events'];

const settle = (page: Page, ms = 1300) => page.waitForTimeout(ms);

const state = (page: Page) =>
	page.evaluate(() => {
		const P = (window as any).__probe;
		const c = document.querySelector('[data-film-canvas]') as any;
		return {
			path: location.pathname,
			title: document.title,
			current: [...document.querySelectorAll('[data-nav-link]')].map((a) => `${(a as HTMLElement).dataset.section}:${a.getAttribute('aria-current') ?? '-'}`).join(' '),
			canvasId: c?.__probeId ?? null,
			ctxTab: +(sessionStorage.getItem('__ctx') || 0),
			lost: P.gl.lost + (P.gl.ctx?.isContextLost() ? 1 : 0),
			contents: document.querySelectorAll('[data-panel-content]').length,
			// Astro's announcer is .astro-route-announcer; any live region counts.
			announcers: document.querySelectorAll('[aria-live]').length,
			scripts: document.scripts.length,
			active: document.activeElement === document.body ? 'body' : `${document.activeElement?.tagName}.${(document.activeElement as HTMLElement)?.dataset?.section ?? (document.activeElement as HTMLElement)?.className}`,
			scrollY: Math.round(scrollY),
			panelTop: Math.round((document.querySelector('[data-panel-content]') as HTMLElement)?.scrollTop ?? -1),
			entries: (window as any).navigation?.entries?.().length ?? history.length,
		};
	});

/** Every check of the prototype's feel.ts, on one side. */
export async function feel(browser: Browser, server: Server, remote: string): Promise<Record<string, unknown>> {
	const R: Record<string, unknown> = {};
	// A check that can't finish, such as a click a full load interrupts, is
	// a result too: it differs from the baseline's.
	const section = async (name: string, body: () => Promise<void>) => {
		try {
			await body();
		} catch (e) {
			R[name] = `failed: ${String((e as Error).message).split('\n')[0]}`;
		}
	};
	const contexts: BrowserContext[] = [];
	const open = async (options: BrowserContextOptions = {}) => {
		const { ctx } = await context(browser, server, remote, { viewport: { width: 1440, height: 900 }, ...options });
		contexts.push(ctx);
		await ctx.addInitScript({ path: join(import.meta.dirname, 'probe.js') });
		return { ctx, page: await ctx.newPage(), base: server.origin };
	};

	// 1. Film continuity over a 12-step tour; aria-current, titles, announcers, scripts.
	await section('tour', async () => {
		const { ctx, page, base } = await open();
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
		const first = await state(page);
		const log: any[] = [];
		for (const [label, act] of tour) {
			await act();
			await settle(page);
			const extra = await page.evaluate(() => ({
				pres: document.querySelectorAll('[data-panel-content] pre').length,
				copyButtons: document.querySelectorAll('[data-panel-content] [data-code-block] > button').length,
				piles: document.querySelectorAll('[data-panel-content] [data-pile]').length,
				pilesInit: document.querySelectorAll('[data-panel-content] [data-pile][data-init]').length,
			}));
			log.push({ label, ...(await state(page)), ...extra });
		}
		R.tour = log.map((s) => `${s.label} -> ${s.path} [${s.current}] contents=${s.contents}`);
		R.filmSameCanvas = log.every((s) => s.canvasId === first.canvasId);
		R.filmContexts = Math.max(...log.map((s) => s.ctxTab));
		R.filmLost = log.reduce((a, s) => a + s.lost, 0);
		R.copyButtonsPerPre = log.filter((s) => s.pres).map((s) => `${s.copyButtons}/${s.pres}`);
		R.photoPileInit = log.filter((s) => s.piles).map((s) => `${s.pilesInit}/${s.piles}`);
		R.announcers = log.map((s) => s.announcers);
		R.titles = log.map((s) => s.title.split(' | ')[0]);
		R.scriptsCount = log.map((s) => s.scripts).join(',');
		R.entries = log[log.length - 1].entries;
		await ctx.close();
	});

	// 2. Window scroll on mobile (the idle page scrolls the window); panel scroll on desktop back.
	await section('mobileWindowScroll', async () => {
		const { ctx, page, base } = await open({ viewport: { width: 390, height: 844 }, isMobile: true, hasTouch: true });
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
	});
	await section('desktopPanelScrollAfterBack', async () => {
		const { ctx, page, base } = await open();
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
	});

	// 3. Reduced motion.
	await section('reducedMotion', async () => {
		const { ctx, page, base } = await open({ reducedMotion: 'reduce' });
		await page.goto(base + '/');
		await settle(page);
		await page.click('[data-nav-link][data-section=writing]');
		await page.waitForFunction(() => document.querySelector('[data-panel-content] .panel-title') && location.pathname.startsWith('/blog'), null, { timeout: 5000 });
		R.reducedMotion = await page.evaluate(() => {
			const panel = document.querySelector('[data-panel]') as HTMLElement;
			const items = [...document.querySelectorAll('[data-panel-content] [data-rv]')] as HTMLElement[];
			return {
				morphAnims: panel.getAnimations().length + ((document.querySelector('[data-film-layer]') as HTMLElement)?.getAnimations().length ?? 0),
				rvBases: [...new Set(items.map((i) => i.style.getPropertyValue('--rv-base') || '(unset)'))].join(','),
				filmReady: panel.dataset.film ?? '(unset)',
				glCalls: (window as any).__probe.gl.calls,
			};
		});
		await settle(page);
		await page.keyboard.press('Escape');
		await page.waitForFunction(() => location.pathname === '/', null, { timeout: 3000 });
		await ctx.close();
	});

	// 4. Focus: keyboard-activate a rail link, then a link inside the panel.
	await section('focus', async () => {
		const { ctx, page, base } = await open();
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
	});

	// 5. Abort: two clicks 5 ms apart; the feed is a full load.
	await section('abort', async () => {
		const { ctx, page, base } = await open();
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
	});
	await Promise.all(contexts.map((c) => c.close().catch(() => {})));
	return R;
}

/** Each result that differs, and whether a Parity exception lets it through. */
export function compareFeel(ours: Record<string, unknown>, theirs: Record<string, unknown>) {
	const out: { check: string; baseline: unknown; candidate: unknown; tolerated?: string }[] = [];
	for (const check of new Set([...Object.keys(ours), ...Object.keys(theirs)])) {
		const [a, b] = [JSON.stringify(ours[check]), JSON.stringify(theirs[check])];
		if (a !== b) out.push({ check, baseline: ours[check], candidate: theirs[check], tolerated: TOLERATED[check] });
	}
	return out;
}
