import { expect, test } from 'vitest';
import type { HTMLElement } from 'node-html-parser';
import { links, main, output, page, routes } from '../../../lib/grade.ts';

/** The selectors of each rule in `css` that rounds corners, without pseudo-classes. */
function rounding(css: string): string[] {
	const found: string[] = [];
	const walk = (text: string) => {
		let depth = 0;
		let start = 0;
		let head = '';
		for (let i = 0; i < text.length; i++) {
			if (text[i] === '{' && depth++ === 0) {
				head = text.slice(start, i).trim();
				start = i + 1;
			} else if (text[i] === '}' && --depth === 0) {
				const inner = text.slice(start, i);
				if (head.startsWith('@')) walk(inner);
				else if (/border(-[a-z]+)*-radius\s*:\s*(?!0(px)?\s*(;|$))/.test(inner)) found.push(...head.split(','));
				start = i + 1;
			}
		}
	};
	walk(css.replace(/\/\*[\s\S]*?\*\//g, ''));
	return found.map((s) => s.replace(/::?[a-zA-Z-]+(\([^()]*\))?/g, '').trim()).filter(Boolean);
}

/** The items and links of lists on the Page at `url` that its stylesheets round. */
function rounded(url: string): HTMLElement[] {
	const root = page(url);
	const css = root
		.querySelectorAll('link[rel="stylesheet"]')
		.map((l) => output(new URL(l.getAttribute('href')!, `https://site.invalid${url}`).pathname) ?? '')
		.join('\n');
	const hits = new Set<HTMLElement>();
	for (const selector of rounding(css)) {
		try {
			for (const el of root.querySelectorAll(selector)) hits.add(el);
		} catch {
			// A selector node-html-parser can't read is one this can't check.
		}
	}
	return [...hits].filter((el) => el.tagName === 'LI' || (el.tagName === 'A' && el.closest('ul, ol')));
}

const href = (el: HTMLElement) => el.getAttribute('href') ?? el.querySelector('a')?.getAttribute('href');

/** The links of the Post's tag list. */
const tags = () => {
	const terms = new Set(routes().filter((r) => r.kind === 'term' && r.taxonomy === 'tags').map((r) => r.url));
	return links(main(page('/blog/hello/')), '/blog/hello/').filter((a) => terms.has(a.href));
};

test("a Post's tags are rounded", () => {
	const round = new Set(rounded('/blog/hello/').map(href));
	expect(tags().length).toBeGreaterThan(0);
	for (const a of tags()) expect(round.has(a.el.getAttribute('href')), `the tag ${a.text}`).toBe(true);
});

test('no other list is', () => {
	for (const url of ['/', '/blog/', '/tags/', '/projects/', '/tags/meta/', '/blog/hello/']) {
		const allowed = new Set(url === '/blog/hello/' ? tags().map((a) => a.el.getAttribute('href')) : []);
		const others = rounded(url).filter((el) => !allowed.has(href(el)));
		expect(others.map((el) => el.outerHTML.slice(0, 100)), url).toEqual([]);
	}
});
