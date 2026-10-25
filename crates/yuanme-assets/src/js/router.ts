// The Router (ADR 0007), which `[router] mode = "swap"` puts on every Page.
// It intercepts same-origin navigations with the Navigation API, fetches
// the next Page and swaps it in. Anything else, such as a browser without
// the Navigation API, a response that isn't a Page of this Site or a
// failed fetch, is an ordinary full page load.

import { mount } from 'yuanme:mount';
import { fire, swap, twin } from 'yuanme:swap';

type Page = { doc: Document; url: URL };

const d = document;
const nav = (window as any).navigation;
// Prefetched responses, by URL, with when they were fetched (#20): Safari
// has neither `<link rel=prefetch>` nor Speculation Rules, and Pages are
// served with max-age=0, so the Router keeps its own.
const prefetched = new Map<string, [number, Promise<Response>]>();
let parser: DOMParser | undefined;
let adopting = false;
let timer = 0;

const skip = (u: URL) => u.origin != location.origin || /\.(?!html?$)\w+$/.test(u.pathname);

export function navigate(url: string, { history = 'auto' }: { history?: 'auto' | 'push' | 'replace' } = {}) {
	if (nav) nav.navigate(url, { history });
	else if (history == 'replace') location.replace(url);
	else location.assign(url);
}

// The Page at `to`, or nothing for a full load instead.
async function load(to: URL, signal: AbortSignal): Promise<Page | undefined> {
	const hit = prefetched.get(to.href);
	const res = await (hit && Date.now() - hit[0] < 3e4 ? hit[1] : fetch(to, { signal }));
	const url = new URL(res.url);
	url.hash = to.hash;
	if (!/^(text\/html|application\/xhtml\+xml)/.test(res.headers.get('content-type') || '') || skip(url)) return;
	const doc = (parser ||= new DOMParser()).parseFromString(await res.clone().text(), 'text/html');
	// Only a Page that runs the Router is one of this Site's.
	if (!doc.querySelector('script[data-yuanme-router]')) return;
	// New stylesheets load before the swap, so the Page never shows unstyled.
	await Promise.all(
		[...doc.querySelectorAll<HTMLLinkElement>('head link[rel=stylesheet]')]
			.filter((l) => !twin(d, l))
			.map(
				(l) =>
					new Promise((r) => {
						const p = Object.assign(d.createElement('link'), { rel: 'preload', as: 'style', href: l.href, onload: r, onerror: r });
						d.head.append(p);
					}),
			),
	);
	return { doc, url };
}

// Adopts a redirected URL without a new entry. Safari aborts the
// navigation for it and fires navigateerror, though the swap goes on.
function adopt(url: URL) {
	if (url.href == location.href) return;
	adopting = true;
	history.replaceState(history.state, '', url);
	adopting = false;
}

if (nav) {
	nav.addEventListener('navigate', (e: any) => {
		const to = new URL(e.destination.url);
		if (
			adopting ||
			!e.canIntercept ||
			e.hashChange ||
			e.downloadRequest != null ||
			e.formData ||
			e.navigationType == 'reload' ||
			skip(to) ||
			e.sourceElement?.closest('[data-yuanme-reload]')
		)
			return;
		const from = new URL(location.href);
		const traverse = e.navigationType == 'traverse';
		let page: Page | undefined;
		const get = async () => {
			try {
				page = await load(to, e.signal);
			} catch {}
		};
		// Where it exists, a redirect commits its final URL as the one
		// entry; a traversal's entry already exists (#12).
		const precommit = e.cancelable && !traverse && 'NavigationPrecommitController' in window;
		e.intercept({
			scroll: 'manual',
			focusReset: 'manual',
			precommitHandler: precommit
				? async (c: any) => {
						await get();
						page && page.url.href != to.href && c.redirect(page.url.href);
					}
				: undefined,
			async handler() {
				precommit || (await get());
				if (e.signal.aborted) return;
				if (!page) return location.reload();
				const { doc, url } = page;
				await swap(doc, from, url, e.navigationType, () => {
					// Firefox 157's scroll() left a push where it was after the
					// swap, so pushes and replaces scroll themselves.
					const target = url.hash && d.getElementById(decodeURIComponent(url.hash.slice(1)));
					if (traverse)
						try {
							e.scroll();
						} catch {}
					else target ? target.scrollIntoView() : scrollTo(0, 0);
					adopt(url);
				});
			},
		});
	});

	// After 80 ms of hover or focus, or at once on touch, as Astro does. A
	// response is kept for 30 s.
	const prefetch = (a: HTMLAnchorElement) => {
		const c = (navigator as any).connection;
		const hit = prefetched.get(a.href);
		if (
			(hit && Date.now() - hit[0] < 3e4) ||
			skip(new URL(a.href)) ||
			a.closest('[data-yuanme-reload],[data-yuanme-prefetch=false]') ||
			c?.saveData ||
			/2g/.test(c?.effectiveType)
		)
			return;
		const res = fetch(a.href);
		res.catch(() => prefetched.delete(a.href));
		prefetched.set(a.href, [Date.now(), res]);
	};
	for (const t of ['mouseover', 'focusin', 'touchstart']) {
		d.addEventListener(
			t,
			(e) => {
				clearTimeout(timer);
				const a = (e.target as Element).closest?.<HTMLAnchorElement>('a[href]');
				if (a) timer = setTimeout(prefetch, t[0] == 't' ? 0 : 80, a);
			},
			{ passive: true },
		);
	}
}

// The first Page mounts and loads once every deferred Script has run, so
// those Scripts' listeners hear it.
d.addEventListener('DOMContentLoaded', () => {
	mount();
	fire('page-load');
});
