import { expect, test } from 'vitest';
import { links, main, page, routes, text } from '../../../lib/grade.ts';
import type { HTMLElement } from 'node-html-parser';

/** The minutes a Post's own Page says it takes, as the Starter shows them. */
const minutes = (url: string) => Number(/(\d+)\s*min/.exec(text(main(page(url))))?.[1]);

/** The row of a list that a link is in. */
const row = (a: HTMLElement) => {
	let el: HTMLElement | null = a;
	while (el && !['LI', 'ARTICLE', 'TR'].includes(el.tagName)) el = el.parentNode;
	return el ?? a.parentNode;
};

test('each Post in the blog list shows its reading time', () => {
	const lists = routes().filter((r) => r.kind === 'list' && r.collection === 'posts');
	const posts = new Set(routes().filter((r) => r.kind === 'entry' && r.collection === 'posts').map((r) => r.url));
	const shown = new Map<string, number>();
	for (const list of lists) {
		for (const a of links(main(page(list.url)), list.url).filter((a) => posts.has(a.href))) {
			shown.set(a.href, Number(/(\d+)\s*min/.exec(text(row(a.el)))?.[1]));
		}
	}
	expect([...shown.keys()].sort()).toEqual([...posts].sort());
	for (const [url, n] of shown) expect(n, url).toBe(minutes(url));
	expect(shown.get('/blog/long-read/')).toBeGreaterThan(shown.get('/blog/hello/')!);
});
