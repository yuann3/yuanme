// The swap core (ADR 0007): it puts a fetched Page into the current
// document. The Router drives it, and the dev client (#68) can too, which
// is why it is a module of its own.

import { mount, unmount } from 'yuanme:mount';

export type SwapEvent = Event & { from: URL; to: URL; navigationType: string; newDocument: Document; swap: () => void };

const d = document;
const PERSIST = 'data-yuanme-persist';
let announcer: HTMLElement | undefined;

export const fire = (name: string) => d.dispatchEvent(new Event('yuanme:' + name));

// Scripts run once per document, keyed on their URL or their text.
const key = (s: HTMLScriptElement) => s.src || s.textContent!;
const ran = new Set<string>();
for (const s of d.scripts) ran.add(key(s));

// Inline modules fire no load event in Chromium, so, as Astro does, a later
// empty data: module stands in for them.
const SENTINEL = 'data:text/javascript,';
const loaded = (s: HTMLScriptElement) => new Promise((r) => (s.onload = s.onerror = r));
function runScripts() {
	const waits: Promise<unknown>[] = [];
	let inline = false;
	for (const s of [...d.scripts]) {
		const k = key(s);
		const t = s.type;
		if (ran.has(k) || (t && t != 'module' && t != 'text/javascript')) continue;
		ran.add(k);
		const n = d.createElement('script');
		for (const { name, value } of s.attributes) n.setAttribute(name, value);
		n.textContent = s.textContent;
		if (n.src) waits.push(loaded(n));
		else inline ||= t == 'module';
		s.replaceWith(n);
	}
	if (inline) {
		const n = Object.assign(d.createElement('script'), { type: 'module', src: SENTINEL });
		waits.push(loaded(n).then(() => n.remove()));
		d.body.append(n);
	}
	return Promise.all(waits);
}

// A stylesheet both heads have stays, so it neither reloads nor flashes.
export const twin = (doc: Document, el: Element) =>
	el.matches('link[rel=stylesheet]') && doc.head.querySelector(`link[rel=stylesheet][href="${el.getAttribute('href')}"]`);

function swapBody(body: HTMLElement) {
	const active = d.activeElement as HTMLElement | null;
	const old = d.body;
	const kept = new Map([...body.querySelectorAll(`[${PERSIST}]`)].map((el) => [el.getAttribute(PERSIST), el]));
	old.replaceWith(body);
	for (const el of old.querySelectorAll(`[${PERSIST}]`)) {
		const to = kept.get(el.getAttribute(PERSIST));
		// One inside another persisted element has moved with it already.
		to?.isConnected && to.replaceWith(el);
	}
	// Moving the focused link blurs it.
	active?.isConnected && active.focus({ preventScroll: true });
}

function announce() {
	const a = (announcer ||= d.createElement('div'));
	a.setAttribute('aria-live', 'assertive');
	a.setAttribute('aria-atomic', 'true');
	a.style.cssText = 'position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%)';
	a.textContent = '';
	d.body.append(a);
	// Screen readers miss text set in the same task as the insertion.
	setTimeout(() => (a.textContent = d.title || d.querySelector('h1')?.textContent || location.pathname), 60);
}

// `commit` runs between the swap and `after-swap`: scroll, and adopting a
// redirected URL.
export async function swap(newDocument: Document, from: URL, to: URL, navigationType: string, commit?: () => void) {
	newDocument.querySelectorAll('noscript').forEach((n) => n.remove());
	const ev = Object.assign(new Event('yuanme:before-swap'), {
		from,
		to,
		navigationType,
		newDocument,
		swap() {
			const r = d.documentElement;
			for (const { name } of [...r.attributes]) r.removeAttribute(name);
			for (const { name, value } of newDocument.documentElement.attributes) r.setAttribute(name, value);
			for (const el of [...d.head.children]) {
				const t = twin(newDocument, el);
				t ? t.remove() : el.remove();
			}
			d.head.append(...newDocument.head.children);
			swapBody(newDocument.body);
		},
	}) as SwapEvent;
	d.dispatchEvent(ev);
	unmount();
	ev.swap();
	commit?.();
	fire('after-swap');
	await runScripts();
	mount();
	fire('page-load');
	announce();
}
