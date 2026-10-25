// The dev client's checks (#68), in a real browser against `yuanme dev`:
// what each kind of save does to the Page on screen, the overlay, console
// forwarding and reloading after a lost connection, on the Router's
// fixture Site; and #15's save-to-paint Gate on a Site the size of the
// Reference site, from writing the file to the frame that shows it.
//
//   node client.ts --yuanme ../../../../target/release/yuanme [--headed] [--runs 50]
//
// The Gate times the Engine too, so it needs a release build. #68 asks for
// headed Chromium; CI runs headless, which rasterizes on the CPU and on an
// M4 Max was the slower of the two at p95 (80 ms against 72 headed, n = 50).
import assert from 'node:assert/strict';
import { type ChildProcess, spawn } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { createInterface } from 'node:readline';
import { parseArgs } from 'node:util';
import { type Browser, type Page, chromium } from 'playwright';

const { values: args } = parseArgs({
	options: {
		yuanme: { type: 'string', default: join(import.meta.dirname, '../../../../target/release/yuanme') },
		headed: { type: 'boolean', default: false },
		runs: { type: 'string', default: '50' },
	},
});
const SITES = join(import.meta.dirname, '../../../yuanme-engine/tests/sites');
const CORPUS = join(import.meta.dirname, '../../../../corpus/md/eyuan-me');
const tmp = mkdtempSync(join(tmpdir(), 'yuanme-client-'));

type Event = Record<string, any>;

/** `yuanme dev --json` in `project`, with each event it prints. */
class Dev {
	events: Event[] = [];
	/** When each event arrived, as `performance.timeOrigin + performance.now()`. */
	arrived: number[] = [];
	origin = '';
	private child: ChildProcess;
	private heard: (() => void)[] = [];

	constructor(project: string, port: string) {
		this.child = spawn(resolve(args.yuanme), ['dev', '--json', '--port', port], {
			cwd: project,
			stdio: ['ignore', 'pipe', 'inherit'],
		});
		createInterface({ input: this.child.stdout! }).on('line', (line) => {
			this.events.push(JSON.parse(line));
			this.arrived.push(performance.timeOrigin + performance.now());
			for (const h of this.heard.splice(0)) h();
		});
	}

	static async start(project: string, port = '0') {
		const dev = new Dev(project, port);
		const ready = await dev.next((e) => e.type == 'server-ready', 0);
		dev.origin = ready.url.replace(/\/$/, '');
		return dev;
	}

	/** The first event from index `from` on that `test` matches. */
	async next(test: (e: Event) => boolean, from = this.events.length): Promise<Event> {
		const deadline = Date.now() + 30_000;
		for (;;) {
			const found = this.events.slice(from).find(test);
			if (found) return found;
			assert.ok(Date.now() < deadline, `no such event in ${JSON.stringify(this.events.slice(from))}`);
			await new Promise<void>((r) => {
				this.heard.push(r);
				setTimeout(r, 1000);
			});
		}
	}

	async stop() {
		const exited = new Promise((r) => this.child.once('exit', r));
		this.child.kill('SIGTERM');
		await exited;
	}
}

function edit(file: string, from: string, to: string) {
	const text = readFileSync(file, 'utf8');
	assert.ok(text.includes(from), `${file} has no ${from}`);
	writeFileSync(file, text.replace(from, to));
}

/** The router fixture with `[router] mode` set to `mode`. */
function routerSite(mode: string) {
	const project = join(tmp, `router-${mode}`);
	cpSync(join(SITES, 'router'), project, { recursive: true });
	edit(join(project, 'yuanme.toml'), 'mode = "swap"', `mode = "${mode}"`);
	return project;
}

/** A Site the size of the Reference site, as `dev::tests::reference` makes it. */
function referenceSite() {
	const root = join(tmp, 'reference');
	cpSync(join(SITES, 'eyuan-sidecars'), root, { recursive: true });
	rmSync(join(root, 'content'), { recursive: true });
	readdirSync(CORPUS)
		.sort()
		.forEach((name, i) => {
			const text = readFileSync(join(CORPUS, name), 'utf8');
			const [, astro, body] = /^---\n([\s\S]*?)\n---\n([\s\S]*)$/.exec(text)!;
			const field = (key: string) =>
				astro
					.split('\n')
					.find((l) => l.startsWith(key + ':'))!
					.slice(key.length + 1)
					.trim()
					.replace(/^['"]+|['"]+$/g, '')
					.replaceAll('"', '\\"');
			const date = `2025-${String((i % 12) + 1).padStart(2, '0')}-01`;
			const [kind, slug] = name.replace(/\.md$/, '').split('--');
			const [dir, extra] = kind == 'blog' ? ['posts', 'tags: [C, Programming]'] : ['projects', 'stack: [go]'];
			const entry = `---\ntitle: "${field('title')}"\ndescription: "${field('description')}"\ndate: ${date}\n${extra}\n---\n${body.replace('(LICENSE)', '(https://example.com/LICENSE)')}`;
			mkdirSync(join(root, 'content', dir), { recursive: true });
			writeFileSync(join(root, 'content', dir, `${slug}.md`), entry);
		});
	// The Reference site runs the Router.
	writeFileSync(join(root, 'yuanme.toml'), readFileSync(join(root, 'yuanme.toml'), 'utf8') + '\n[router]\nmode = "swap"\n');
	return root;
}

const state = (page: Page) =>
	page.evaluate(() => {
		const w = window as any;
		return {
			marker: w.__marker as number | undefined,
			events: [...(w.__events ?? [])] as string[],
			life: [...(w.__life ?? [])] as string[],
			top: w.__top as number,
			canvas: (document.querySelector('[data-film-canvas]') as any)?.__probeId as number,
			scrollY: Math.round(scrollY),
			pres: document.querySelectorAll('main pre').length,
			copies: document.querySelectorAll('main pre > button.copy').length,
			sheets: document.querySelectorAll('link[rel=stylesheet]').length,
			counter: document.querySelector('[data-counter]')?.textContent ?? null,
			overlay: document.querySelector('yuanme-overlay')?.shadowRoot?.textContent ?? null,
		};
	});

/** Opens `path` and waits for its first `yuanme:page-load`, or with the Router off, for its load. */
async function open(page: Page, dev: Dev, path: string) {
	await page.goto(dev.origin + path);
	await page.waitForFunction(() => document.readyState == 'complete' && (window as any).__life?.length);
	// The client reads the Page once it has loaded.
	await page.waitForTimeout(200);
	await page.evaluate(() => ((window as any).__marker = 1));
}

const until = (page: Page, test: () => boolean) => page.waitForFunction(test, undefined, { timeout: 10_000 });
const fullLoad = async (page: Page) => {
	await page.waitForFunction(() => (window as any).__marker === undefined && document.readyState == 'complete', undefined, {
		timeout: 10_000,
	});
};

type Check = (page: Page, dev: Dev, project: string) => Promise<void>;

const CHECKS: Record<string, Check> = {
	// A prose edit swaps the Page in place: what the Site keeps survives,
	// the scroll stays, and the Sidecar scripts are cleaned up and mounted
	// on the new body.
	async prose(page, dev, project) {
		await open(page, dev, '/posts/long/');
		await page.evaluate(() => scrollTo(0, 400));
		const before = await state(page);
		edit(join(project, 'content/posts/long.md'), 'Paragraph 2 of', 'Paragraph two of');
		await until(page, () => document.body.textContent!.includes('Paragraph two of'));
		await until(page, () => (window as any).__events.at(-1) == 'page-load');
		const s = await state(page);
		assert.equal(s.marker, 1, 'a full load');
		assert.equal(s.canvas, before.canvas, 'the persisted canvas');
		assert.equal(s.scrollY, 400, 'the scroll');
		assert.deepEqual(s.events.slice(before.events.length), ['before-swap', 'after-swap', 'page-load']);
		assert.deepEqual(s.life.slice(before.life.length), ['abort /posts/long/', 'cleanup /posts/long/', `mount /posts/long/ 2`]);
		assert.equal(s.copies, s.pres, 'mounted on the new body');
	},

	// A stylesheet edit swaps the stylesheet alone.
	async css(page, dev, project) {
		await open(page, dev, '/about/');
		const before = await state(page);
		edit(join(project, 'styles/site.css'), 'body {', 'h1 { color: rgb(1, 2, 3); }\nbody {');
		await until(page, () => getComputedStyle(document.querySelector('h1')!).color == 'rgb(1, 2, 3)');
		await page.waitForTimeout(200);
		const s = await state(page);
		assert.equal(s.marker, 1, 'a full load');
		assert.deepEqual(s.events, before.events, 'a swap');
		assert.deepEqual(s.life, before.life, 'a mount');
		assert.equal(s.sheets, before.sheets, 'the old stylesheet is gone');
	},

	// A Sidecar script's edit mounts its new code in place, and the Router
	// stays one Router: the next Soft navigation swaps once.
	async sidecar(page, dev, project) {
		await open(page, dev, '/');
		const before = await state(page);
		assert.match(before.counter!, /^mounted \d+$/);
		edit(join(project, 'templates/components/counter.ts'), '`mounted ${', '`counted ${');
		await until(page, () => document.querySelector('[data-counter]')?.textContent?.startsWith('counted') ?? false);
		edit(join(project, 'templates/layouts/base.ts'), 'w.__life.push(`mount ${', 'w.__life.push(`remount ${');
		await until(page, () => (window as any).__life.at(-1)?.startsWith('remount /'));
		const s = await state(page);
		assert.equal(s.marker, 1, 'a full load');
		assert.equal(s.counter, 'counted 1', "the new module's own ctx.data");
		assert.equal(s.top, 2, "the Layout's Sidecar script ran once more, as a new module");
		assert.deepEqual(s.life.slice(-3), ['abort /', 'cleanup /', 'remount / 1']);
		await page.click('[data-nav-link][data-section=about]');
		await until(page, () => location.pathname == '/about/' && (window as any).__events.at(-1) == 'page-load');
		await page.waitForTimeout(200);
		const after = await state(page);
		assert.deepEqual(after.events.slice(s.events.length), ['before-swap', 'after-swap', 'page-load'], 'one Router');
	},

	// An error shows the overlay over the last good build, and the fix
	// hides it.
	async overlay(page, dev, project) {
		await open(page, dev, '/about/');
		const template = join(project, 'templates/about.html');
		const good = readFileSync(template, 'utf8');
		edit(template, '<h1>About</h1>', '<h1>{{ nope( }}</h1>');
		await until(page, () => !!document.querySelector('yuanme-overlay'));
		const s = await state(page);
		assert.match(s.overlay!, /1 error/);
		assert.match(s.overlay!, /error\[template\/[a-z-]+\]/);
		assert.match(s.overlay!, /templates\/about\.html/);
		assert.equal(await page.textContent('h1'), 'About', 'the last good build');
		writeFileSync(template, good.replace('<h1>About</h1>', '<h1>About us</h1>'));
		await until(page, () => !document.querySelector('yuanme-overlay') && document.querySelector('h1')?.textContent == 'About us');
		assert.equal((await state(page)).marker, 1, 'a full load');
	},

	// The console and uncaught errors reach the terminal.
	async console(page, dev) {
		await open(page, dev, '/about/');
		const from = dev.events.length;
		await page.evaluate(() => {
			console.warn('from the page', { n: 1 });
			setTimeout(() => {
				throw new Error('boom');
			});
		});
		const warned = await dev.next((e) => e.type == 'browser-console' && e.level == 'warn', from);
		assert.deepEqual(warned, { type: 'browser-console', level: 'warn', message: 'from the page {"n":1}', url: '/about/', stack: null });
		const thrown = await dev.next((e) => e.type == 'browser-console' && e.level == 'error', from);
		assert.equal(thrown.message, 'Uncaught Error: boom');
		assert.match(thrown.stack, /boom/);
	},

	// A Site Script's edit is a full load: its old copy would go on running.
	async script(page, dev, project) {
		await open(page, dev, '/about/');
		edit(join(project, 'scripts/site.ts'), 'w.__events = [];', 'w.__events = [];\nw.__edited = 1;');
		await fullLoad(page);
		await until(page, () => (window as any).__edited == 1);
	},
};

// Without the Router nothing binds a swapped Page, so only a stylesheet
// changes in place.
async function routerOff(page: Page) {
	const project = routerSite('off');
	const dev = await Dev.start(project);
	try {
		await open(page, dev, '/about/');
		edit(join(project, 'styles/site.css'), 'body {', 'h1 { color: rgb(4, 5, 6); }\nbody {');
		await until(page, () => getComputedStyle(document.querySelector('h1')!).color == 'rgb(4, 5, 6)');
		assert.equal((await state(page)).marker, 1, 'a full load for a stylesheet');
		edit(join(project, 'templates/about.html'), '<h1>About</h1>', '<h1>About us</h1>');
		await fullLoad(page);
		assert.equal(await page.textContent('h1'), 'About us');
	} finally {
		await dev.stop();
	}
}

// A dev server that went away and came back may have built anything, so
// the client loads the Page again.
async function reconnect(page: Page) {
	const project = routerSite('swap');
	const first = await Dev.start(project);
	const port = new URL(first.origin).port;
	await open(page, first, '/about/');
	await first.stop();
	const second = await Dev.start(project, port);
	try {
		await fullLoad(page);
	} finally {
		await second.stop();
	}
}

const pct = (xs: number[], p: number) => [...xs].sort((a, b) => a - b)[Math.ceil(xs.length * p) - 1];

/**
 * Writes `change(i)` to `file` and times until `shown(i)` holds in a frame
 * that has painted, `runs` times, and until the `reload` event, which is
 * the dev server's half.
 */
async function time(page: Page, dev: Dev, file: string, change: (text: string, i: number) => string, shown: string, runs: number) {
	const text = readFileSync(file, 'utf8');
	const times: number[] = [];
	const server: number[] = [];
	for (let i = 0; i < runs + 5; i++) {
		await page.waitForTimeout(150);
		await page.evaluate(
			([shown, i]) => {
				const test = new Function('i', `return ${shown}`) as (i: number) => boolean;
				(window as any).__painted = new Promise((done) => {
					const frame = () => {
						if (!test(i)) return requestAnimationFrame(frame);
						// After this frame's paint.
						const c = new MessageChannel();
						c.port1.onmessage = () => done(performance.timeOrigin + performance.now());
						c.port2.postMessage(0);
					};
					requestAnimationFrame(frame);
				});
			},
			[shown, i] as const,
		);
		const from = dev.events.length;
		const saved = performance.timeOrigin + performance.now();
		writeFileSync(file, change(text, i));
		const painted: number = await page.evaluate(() => (window as any).__painted);
		const reload = dev.events.indexOf(await dev.next((e) => e.type == 'reload', from));
		// The first saves warm what the first build didn't.
		if (i >= 5) {
			times.push(painted - saved);
			server.push(dev.arrived[reload] - saved);
		}
	}
	writeFileSync(file, text);
	assert.equal(await page.evaluate(() => (window as any).__marker), 1, 'a full load');
	return { times, server };
}

async function gate(page: Page, runs: number) {
	const project = referenceSite();
	const dev = await Dev.start(project);
	try {
		await page.goto(dev.origin + '/blog/extending_c_stdlib/');
		await page.waitForFunction(() => document.readyState == 'complete');
		await page.waitForTimeout(500);
		await page.evaluate(() => ((window as any).__marker = 1));
		const results = {
			prose: await time(
				page,
				dev,
				join(project, 'content/posts/extending_c_stdlib.md'),
				(text, i) => text.replace('\n\n', `\n\nYuanme edit ${i}.\n\n`),
				"document.body.textContent.includes(`Yuanme edit ${i}.`)",
				runs,
			),
			stylesheet: await time(
				page,
				dev,
				join(project, 'styles/global.css'),
				(text, i) => `${text}\n:root { --yuanme-edit: ${i}; }\n`,
				"getComputedStyle(document.documentElement).getPropertyValue('--yuanme-edit').trim() == String(i)",
				runs,
			),
		};
		let over = false;
		const ms = (xs: number[]) => `${pct(xs, 0.5).toFixed(1)} ms p50, ${pct(xs, 0.95).toFixed(1)} ms p95, ${Math.max(...xs).toFixed(1)} ms at most`;
		for (const [edit, { times, server }] of Object.entries(results)) {
			console.log(`save-to-paint, ${edit} edit of the long post: ${ms(times)} (n = ${times.length})`);
			console.log(`  of which save to the reload event: ${ms(server)}`);
			over ||= pct(times, 0.95) > 100;
		}
		assert.ok(!over, 'save-to-paint is over 100 ms at p95');
	} finally {
		await dev.stop();
	}
}

let failed = 0;
const browser: Browser = await chromium.launch({ headless: !args.headed });
console.log(`chromium ${browser.version()}, ${args.headed ? 'headed' : 'headless'}`);
const run = async (name: string, check: (page: Page) => Promise<void>) => {
	const context = await browser.newContext({ viewport: { width: 1280, height: 800 } });
	context.setDefaultTimeout(10_000);
	try {
		await check(await context.newPage());
		console.log(`ok   ${name}`);
	} catch (e) {
		failed++;
		console.log(`FAIL ${name}: ${(e as Error).stack}`);
	} finally {
		await context.close();
	}
};
try {
	const project = routerSite('swap');
	const dev = await Dev.start(project);
	try {
		for (const [name, check] of Object.entries(CHECKS)) await run(name, (page) => check(page, dev, project));
	} finally {
		await dev.stop();
	}
	await run('routerOff', routerOff);
	await run('reconnect', reconnect);
	await run('gate', (page) => gate(page, Number(args.runs)));
} finally {
	await browser.close();
	rmSync(tmp, { recursive: true, force: true });
}
if (failed) {
	console.log(`${failed} failed`);
	process.exit(1);
}
