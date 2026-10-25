// The pixel check (#23): every Page × 390, 900 and 1280 px × light and
// dark, screenshotted with reduced motion once animations have settled, and
// compared with pixelmatch.
import { mkdirSync, writeFileSync } from 'node:fs';
import { availableParallelism } from 'node:os';
import { join } from 'node:path';
import pixelmatch from 'pixelmatch';
import { PNG } from 'pngjs';
import type { Browser, BrowserContext, Page } from 'playwright';
import { context, type Server } from './lib.ts';

/** The ≤ 767, 768–1099 and ≥ 1100 px bands. Heights aren't set by #23; these are a phone's, a tablet's and a laptop's. */
export const VIEWPORTS = [
	{ width: 390, height: 844 },
	{ width: 900, height: 1024 },
	{ width: 1280, height: 800 },
];
export const SCHEMES = ['light', 'dark'] as const;
/** pixelmatch 8's per-pixel threshold, an OKLab HyAB distance. */
export const THRESHOLD = 0.1;
/** Re-encoded photos: the About photos, and hero images elsewhere. */
export const PHOTO_THRESHOLD = 0.25;
/** A screenshot fails if more than 0.05 % of its pixels differ. */
export const MAX_RATIO = 0.0005;
export const RULES = ['about-image-bytes', 'hero-images'];
/** A long post's panel is about 20 screens; this bounds a runaway one. */
const MAX_FRAMES = 60;

type Rect = { x: number; y: number; w: number; h: number };
type Shot = { png: PNG; photos: Rect[] };
export type Side = { server: Server; remote: string; cache?: Map<string, Shot[]> };
export type Failure = { page: string; viewport: string; reason: string };

/** Fonts, images and finite animations done. */
async function settle(page: Page) {
	await page.evaluate(async () => {
		for (const img of document.images) img.loading = 'eager';
		await document.fonts.ready;
		await Promise.all([...document.images].map((img) => img.decode().catch(() => {})));
		const busy = () =>
			document.getAnimations().some((a) => a.playState === 'running' && Number.isFinite(a.effect?.getComputedTiming().endTime));
		const start = performance.now();
		while (busy() && performance.now() - start < 5000) await new Promise((r) => requestAnimationFrame(r));
		await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
	});
}

/**
 * Where each raster image is drawn, in page coordinates, clipped by the
 * scroll containers it is in. SVGs are drawn by the browser on both sides,
 * so they keep the strict threshold.
 */
function photos(page: Page): Promise<Rect[]> {
	return page.evaluate(() => {
		const out = [];
		for (const img of document.images) {
			if (!img.currentSrc || /\.svg(\?|#|$)/i.test(img.currentSrc) || img.currentSrc.startsWith('data:image/svg')) continue;
			let { left, top, right, bottom } = img.getBoundingClientRect();
			for (let el = img.parentElement; el; el = el.parentElement) {
				if (getComputedStyle(el).overflow === 'visible') continue;
				const c = el.getBoundingClientRect();
				left = Math.max(left, c.left);
				top = Math.max(top, c.top);
				right = Math.min(right, c.right);
				bottom = Math.min(bottom, c.bottom);
			}
			if (right > left && bottom > top) out.push({ x: left + scrollX, y: top + scrollY, w: right - left, h: bottom - top });
		}
		return out;
	});
}

/**
 * A Page's screenshots: the full page, and once per screen of the panel,
 * which scrolls inside the window on wide viewports.
 */
async function shoot(ctx: BrowserContext, origin: string, path: string): Promise<Shot[]> {
	const page = await ctx.newPage();
	await page.goto(origin + path, { waitUntil: 'load' });
	await settle(page);
	const frames = await page.evaluate((max) => {
		const p = document.querySelector('[data-panel-content]');
		return p && p.clientHeight ? Math.min(max, Math.ceil(p.scrollHeight / p.clientHeight)) : 1;
	}, MAX_FRAMES);
	const shots: Shot[] = [];
	for (let i = 0; i < frames; i++) {
		if (i) {
			await page.evaluate((i) => {
				const p = document.querySelector('[data-panel-content]') as HTMLElement;
				p.scrollTop = i * p.clientHeight;
			}, i);
			await settle(page);
		}
		const png = PNG.sync.read(await page.screenshot({ fullPage: true, animations: 'disabled', caret: 'hide' }));
		shots.push({ png, photos: await photos(page) });
	}
	await page.close();
	return shots;
}

/** Differing pixels: the strict threshold outside photos, the photo threshold inside them. */
function differing(a: Shot, b: Shot, diff: PNG): number {
	const { width, height } = a.png;
	const photo = new Uint8Array(width * height);
	for (const r of [...a.photos, ...b.photos]) {
		for (let y = Math.max(0, Math.floor(r.y)); y < Math.min(height, Math.ceil(r.y + r.h)); y++) {
			photo.fill(1, y * width + Math.max(0, Math.floor(r.x)), y * width + Math.min(width, Math.ceil(r.x + r.w)));
		}
	}
	const rest = photo.map((p) => 1 - p);
	return (
		pixelmatch(a.png.data, b.png.data, diff.data, width, height, { threshold: THRESHOLD, ignoreMask: photo }) +
		pixelmatch(a.png.data, b.png.data, null, width, height, { threshold: PHOTO_THRESHOLD, ignoreMask: rest })
	);
}

/** Runs `f` on every item, `n` at a time. */
async function pool<T, R>(items: T[], n: number, f: (item: T) => Promise<R>): Promise<R[]> {
	const out: R[] = [];
	let next = 0;
	await Promise.all(
		Array.from({ length: n }, async () => {
			while (next < items.length) {
				const i = next++;
				out[i] = await f(items[i]);
			}
		}),
	);
	return out;
}

/** Compares `paths` on both sides in one engine, writing each failing screenshot's pair and diff under `out`. */
export async function pixels(browser: Browser, engine: string, baseline: Side, candidate: Side, paths: string[], out: string): Promise<Failure[]> {
	const matrix = VIEWPORTS.flatMap((viewport) => SCHEMES.map((colorScheme) => ({ viewport, colorScheme })));
	const failures = await pool(matrix, Math.min(3, availableParallelism()), async ({ viewport, colorScheme }) => {
		const failures: Failure[] = [];
		const options = { viewport, colorScheme, reducedMotion: 'reduce' as const, deviceScaleFactor: 1 };
		const b = await context(browser, baseline.server, baseline.remote, options);
		const c = await context(browser, candidate.server, candidate.remote, options);
		const name = `${viewport.width}px ${colorScheme}`;
		let screens = 0;
		for (const path of paths) {
			const key = `${engine} ${name} ${path}`;
			const [ours, theirs] = await Promise.all([baseline.cache?.get(key) ?? shoot(b.ctx, baseline.server.origin, path), shoot(c.ctx, candidate.server.origin, path)]);
			baseline.cache?.set(key, ours);
			screens += ours.length;
			if (ours.length !== theirs.length) {
				failures.push({ page: path, viewport: name, reason: `the panel is ${ours.length} screens long in the baseline, ${theirs.length} in the candidate` });
				continue;
			}
			for (let i = 0; i < ours.length; i++) {
				const [x, y] = [ours[i].png, theirs[i].png];
				const frame = ours.length > 1 ? ` screen ${i + 1}` : '';
				if (x.width !== y.width || x.height !== y.height) {
					failures.push({ page: path, viewport: name, reason: `${frame}: ${x.width}×${x.height} in the baseline, ${y.width}×${y.height} in the candidate` });
					continue;
				}
				const diff = new PNG({ width: x.width, height: x.height });
				const ratio = differing(ours[i], theirs[i], diff) / (x.width * x.height);
				if (ratio <= MAX_RATIO) continue;
				const file = join(out, engine, `${path.replace(/\W+/g, '_')}${viewport.width}-${colorScheme}-${i + 1}`);
				mkdirSync(join(out, engine), { recursive: true });
				writeFileSync(`${file}-baseline.png`, PNG.sync.write(x));
				writeFileSync(`${file}-candidate.png`, PNG.sync.write(y));
				writeFileSync(`${file}-diff.png`, PNG.sync.write(diff));
				failures.push({ page: path, viewport: name, reason: `${frame}: ${(ratio * 100).toFixed(3)} % of pixels differ (${file}-diff.png)` });
			}
		}
		for (const [side, refused] of [['baseline', b.refused], ['candidate', c.refused]] as const) {
			for (const url of new Set(refused)) failures.push({ page: '', viewport: name, reason: `the ${side} requested ${url}, which isn't in the baseline's remote/` });
		}
		await b.ctx.close();
		await c.ctx.close();
		console.log(`${engine} ${name}: ${paths.length} Pages, ${screens} screens`);
		return failures;
	});
	return failures.flat();
}
