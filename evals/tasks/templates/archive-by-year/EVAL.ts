import { expect, test } from 'vitest';
import { main, page, pages, routes, text } from '../../../lib/grade.ts';

/** Each Post's URL and the year its Page shows it was published. */
const years = () =>
	new Map(
		routes()
			.filter((r) => r.kind === 'entry' && r.collection === 'posts')
			.map((r) => [r.url, page(r.url).querySelector('time[datetime]')!.getAttribute('datetime')!.slice(0, 4)]),
	);

/** On each Page that has headings that are years, the Posts linked under each, in order. */
const archives = () => {
	const posts = years();
	return pages().flatMap(({ url, html }) => {
		const groups: [string, string[]][] = [];
		for (const el of main(html).querySelectorAll('h2, h3, h4, a[href]')) {
			if (el.tagName !== 'A') {
				if (/^\d{4}$/.test(text(el))) groups.push([text(el), []]);
				continue;
			}
			const to = new URL(el.getAttribute('href')!, `https://site.invalid${url}`).pathname;
			if (groups.length && posts.has(to)) groups[groups.length - 1][1].push(to);
		}
		return groups.length > 1 ? [{ url, groups }] : [];
	});
};

test('a Page lists every Post under the year it was published', () => {
	const found = archives();
	expect(found.map((a) => a.url)).toHaveLength(1);
	const expected = new Map<string, string[]>();
	for (const [url, year] of years()) expected.set(year, [...(expected.get(year) ?? []), url].sort());
	const shown = new Map(found[0].groups.map(([year, urls]) => [year, [...new Set(urls)].sort()]));
	expect(shown).toEqual(expected);
});

test('newest year first', () => {
	const shown = archives()[0]?.groups.map(([year]) => year) ?? [];
	expect(shown).toEqual([...shown].sort().reverse());
});
