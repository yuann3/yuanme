import { expect, test } from 'vitest';
import { links, main, page, routes } from '../../../lib/grade.ts';

/** The ids of a Post's sections, and the links before its first section to each of them. */
function contents(url: string) {
	const body = main(page(url));
	const ids = body.querySelectorAll('h2').map((h) => h.id).filter(Boolean);
	const first = body.querySelector('h2');
	const all = body.querySelectorAll('*');
	const before = first ? all.slice(0, all.indexOf(first)) : all;
	const linked = links(body, url)
		.filter((a) => a.href === url && ids.includes(a.hash) && before.includes(a.el))
		.map((a) => a.hash);
	return { ids, linked };
}

const posts = () => routes().filter((r) => r.kind === 'entry' && r.collection === 'posts').map((r) => r.url);

test('a Post with more than two sections starts with a link to each, in order', () => {
	const long = posts().filter((url) => contents(url).ids.length > 2);
	expect(long.length).toBeGreaterThan(0);
	for (const url of long) expect(contents(url).linked, url).toEqual(contents(url).ids);
});

test('a Post with two sections has none', () => {
	const short = posts().filter((url) => contents(url).ids.length <= 2);
	expect(short).toContain('/blog/two-sections/');
	for (const url of short) expect(contents(url).linked, url).toEqual([]);
});
