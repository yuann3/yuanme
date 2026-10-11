// Optional History API driver, for browsers without the Navigation API
// (Firefox < 147, Safari < 26.2). Prototype only: built into the
// "proto-hist" variant to measure what it costs.

import { load, skip, swap, useFallback } from './router';

const w = window as any;
if (!w.navigation && document.querySelector('[data-shell]')) {
	let cur = new URL(location.href);
	let ctrl: AbortController | undefined;
	let t = 0;
	history.scrollRestoration = 'manual';
	const save = () => history.replaceState({ x: scrollX, y: scrollY }, '');
	addEventListener('scroll', () => {
		clearTimeout(t);
		t = window.setTimeout(save, 100);
	}, { passive: true });

	const go = async (to: URL, state?: { x: number, y: number }) => {
		ctrl?.abort();
		const signal = (ctrl = new AbortController()).signal;
		let page;
		try {
			page = await load(to, signal);
		} catch {}
		if (signal.aborted) return;
		if (!page) return state ? location.reload() : location.assign(to);
		const from = cur;
		cur = page.url;
		await swap(page.doc, from, page.url, state ? 'traverse' : 'push', () => {
			if (!state) history.pushState({ x: 0, y: 0 }, '', page.url);
			else history.replaceState(state, '', page.url);
			scrollTo(state?.x || 0, state?.y || 0);
		});
	};
	useFallback((href) => go(new URL(href, location.href)));

	document.addEventListener('click', (e) => {
		const a = (e.target as Element).closest?.<HTMLAnchorElement>('a[href]');
		if (!a || e.defaultPrevented || e.button || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey ||
			(a.target && a.target !== '_self') || a.hasAttribute('download')) return;
		const to = new URL(a.href);
		if (skip(to) || (to.hash && to.pathname === location.pathname && to.search === location.search)) return;
		e.preventDefault();
		save();
		go(to);
	});
	addEventListener('popstate', (e) => {
		if (location.pathname + location.search !== cur.pathname + cur.search) go(new URL(location.href), e.state || { x: 0, y: 0 });
	});
}
