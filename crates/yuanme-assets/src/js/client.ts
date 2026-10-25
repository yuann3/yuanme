// The dev client (#68), which only `yuanme dev` serves, on every HTML Page.
// It hears what a save changed over the dev server's WebSocket and shows
// it: a stylesheet is swapped where it is, a Page with the Router is
// swapped in place by the Router's own swap core (ADR 0007), which mounts
// its Sidecar scripts again (ADR 0013), and anything else is a full load.
// It shows the overlay while the build has errors, and sends the console
// to the terminal.

import { swap } from 'yuanme:swap';

type Level = 'log' | 'info' | 'warn' | 'error' | 'debug';
// What a Page was when the client last read it, to tell what an edit
// changed: everything but its stylesheets, and the Scripts it runs.
type Seen = { path: string; shape: string; scripts: string[] };

const d = document;
const parser = new DOMParser();
let socket: WebSocket | undefined;
const queue: string[] = [];
let lost = false;
let work = Promise.resolve();
let overlay: HTMLElement | undefined;

function post(level: Level, message: string, stack?: string) {
	const text = JSON.stringify({ type: 'browser-console', level, message, url: location.pathname, stack: stack ?? null });
	socket ? socket.send(text) : queue.push(text);
}

const text = (v: unknown) => {
	if (typeof v == 'string') return v;
	if (v instanceof Error) return String(v);
	try {
		return JSON.stringify(v) ?? String(v);
	} catch {
		return String(v);
	}
};

for (const level of ['log', 'info', 'warn', 'error', 'debug'] as Level[]) {
	const original = console[level].bind(console);
	console[level] = (...args: unknown[]) => {
		original(...args);
		post(level, args.map(text).join(' '), args.find((a): a is Error => a instanceof Error)?.stack);
	};
}
addEventListener('error', (e) => post('error', 'Uncaught ' + (e.error ? text(e.error) : e.message), e.error?.stack));
addEventListener('unhandledrejection', (e) => post('error', 'Uncaught (in promise) ' + text(e.reason), e.reason?.stack));

const parse = async (res: Response) => parser.parseFromString(await res.text(), 'text/html');
const get = () => fetch(location.href, { cache: 'no-store' });
const sheets = (doc: Document) => [...doc.querySelectorAll<HTMLLinkElement>('link[rel=stylesheet]')];
// The client's own tag and a Page Script, which the Router marks, may
// change without a full load.
const scripts = (doc: Document) =>
	[...doc.scripts].filter((s) => !s.matches('[data-yuanme-client],[data-yuanme-router]')).map((s) => s.src || s.text);
const see = (doc: Document, path: string): Seen => ({
	path,
	shape:
		[...doc.documentElement.attributes].map((a) => a.name + '=' + a.value).join() +
		[...doc.head.children].filter((e) => !e.matches('link[rel=stylesheet]')).map((e) => e.outerHTML).join('') +
		doc.body.outerHTML,
	scripts: scripts(doc),
});
const pathOf = (url: string) => new URL(url, location.href).pathname;

// Read now, before any edit: the live document is what Scripts made of it.
let seen: Promise<Seen | undefined>;
let at = '';
const look = () => {
	const path = (at = location.pathname);
	seen = get().then(parse).then((doc) => see(doc, path), () => undefined);
};
look();
// After a Soft navigation; a swap of the same Page keeps what it saw.
d.addEventListener('yuanme:page-load', () => location.pathname == at || look());

const loaded = (link: HTMLLinkElement) => new Promise((r) => (link.onload = link.onerror = r));

// Each stylesheet of `want`, by href, in its place: a new one goes in
// after the one before it and loads before the old ones go.
async function restyle(want: string[]) {
	const now = sheets(d);
	let before: Element | undefined;
	const added: Promise<unknown>[] = [];
	for (const href of want) {
		const old = now.find((l) => l.href == href);
		if (old) {
			before = old;
			continue;
		}
		const link = Object.assign(d.createElement('link'), { rel: 'stylesheet', href });
		added.push(loaded(link));
		before ? before.after(link) : d.head.append(link);
		before = link;
	}
	await Promise.all(added);
	for (const l of now) want.includes(l.href) || l.remove();
}

async function update(urls: string[]) {
	const changed = new Set(urls.map(pathOf));
	const here = location.pathname;
	if (!changed.has(here)) {
		// A Static file the Page loads, whose URL has no hash.
		for (const el of d.querySelectorAll<HTMLElement>('link[href],[src]')) {
			const at = (el as HTMLLinkElement).href || (el as HTMLImageElement).src;
			if (!at || !changed.has(pathOf(at))) continue;
			if (!el.matches('link[rel=stylesheet]')) return location.reload();
			const fresh = new URL(at);
			fresh.searchParams.set('yuanme', String(Date.now()));
			await restyle(sheets(d).map((l) => (l == el ? fresh.href : l.href)));
		}
		return;
	}
	const before = await seen;
	const res = await get();
	if (!res.ok) return location.reload();
	const doc = await parse(res);
	const after = see(doc, here);
	seen = Promise.resolve(after);
	if (before?.path != here) return location.reload();
	if (before.shape == after.shape) return restyle(sheets(doc).map((l) => l.href));
	// A Script that ran and is gone or changed would go on running beside
	// its new copy, and without the Router nothing binds a swapped Page.
	const router = '[data-yuanme-router]';
	if (!d.querySelector(router) || !doc.querySelector(router) || before.scripts.some((s) => !after.scripts.includes(s))) {
		return location.reload();
	}
	// New stylesheets load first, so the Page never shows unstyled.
	await Promise.all(
		sheets(doc)
			.filter((l) => !sheets(d).some((o) => o.href == l.href))
			.map((l) => {
				const p = Object.assign(d.createElement('link'), { rel: 'preload', as: 'style', href: l.href });
				d.head.append(p);
				return loaded(p);
			}),
	);
	const url = new URL(location.href);
	const [x, y] = [scrollX, scrollY];
	await swap(doc, url, url, 'reload', () => scrollTo(x, y));
}

const close = () => {
	overlay?.remove();
	overlay = undefined;
};
d.addEventListener('keydown', (e) => overlay && e.key == 'Escape' && close());

function showOverlay(diagnostics: string[], failed: boolean) {
	close();
	if (!diagnostics.length) return;
	overlay = d.createElement('yuanme-overlay');
	const root = overlay.attachShadow({ mode: 'open' });
	root.innerHTML = `<style>
:host{all:initial;position:fixed;inset:0;z-index:2147483647;display:block;overflow:auto;background:#000c;color:#f5f5f5;font:14px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace}
div{max-width:960px;margin:48px auto;padding:24px;background:#18181b;border-top:4px solid #f87171;border-radius:6px}
h2{margin:0 0 4px;font-size:16px;color:#fca5a5}p{margin:0 0 16px;color:#d4d4d8}
pre{margin:0;white-space:pre-wrap;overflow-wrap:anywhere}
button{float:right;font:inherit;color:inherit;background:#3f3f46;border:0;border-radius:4px;padding:2px 10px;cursor:pointer}
button:focus-visible{outline:2px solid #93c5fd;outline-offset:2px}
</style><div role="dialog" aria-labelledby="t" aria-describedby="m"><button type="button" aria-label="Close">Close</button><h2 id="t"></h2><p id="m"></p><pre></pre></div>`;
	const n = diagnostics.length;
	root.querySelector('h2')!.textContent = `${n} ${n == 1 ? 'error' : 'errors'}`;
	root.querySelector('p')!.textContent = failed
		? 'The build failed, so this is the last build that succeeded. Saving a fix clears this.'
		: 'The Site built, but its Scripts have type errors. Saving a fix clears this.';
	root.querySelector('pre')!.textContent = diagnostics.join('\n\n');
	root.querySelector('button')!.onclick = close;
	// Outside `<body>`, which a swap replaces.
	d.documentElement.append(overlay);
}

function connect() {
	const s = new WebSocket(`ws://${location.host}/__yuanme/ws`);
	s.onopen = () => {
		// The server restarted, or dropped a client that fell behind: what
		// it missed is unknown.
		if (lost) return location.reload();
		socket = s;
		for (const m of queue.splice(0)) s.send(m);
	};
	s.onmessage = (e) => {
		const m = JSON.parse(e.data);
		if (m.type == 'overlay') showOverlay(m.diagnostics, m.failed);
		if (m.type == 'reload') work = work.then(() => update(m.urls)).catch(() => location.reload());
	};
	s.onclose = () => {
		socket = undefined;
		lost = true;
		setTimeout(connect, 300);
	};
}
connect();
