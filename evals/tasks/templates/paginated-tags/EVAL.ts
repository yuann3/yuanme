import { expect, test } from 'vitest';
import { links, main, page, routes } from '../../../lib/grade.ts';

const posts = () => new Set(routes().filter((r) => r.kind === 'entry' && r.collection === 'posts').map((r) => r.url));
const listed = (url: string) => links(main(page(url)), url).map((a) => a.href).filter((href) => posts().has(href));
/** The Pages of the tag `notes`. */
const notes = () => routes().filter((r) => r.kind === 'term' && r.taxonomy === 'tags' && r.term === 'notes').map((r) => r.url);

test('each Page of a tag lists at most 5 Posts', () => {
	expect(notes().length).toBeGreaterThan(1);
	for (const url of notes()) expect(new Set(listed(url)).size, url).toBeLessThanOrEqual(5);
});

test('together they list all 12 of its Posts', () => {
	expect(new Set(notes().flatMap(listed)).size).toBe(12);
});

test('from its first Page, links lead to every other', () => {
	const all = new Set(notes());
	const first = [...all].sort((a, b) => a.length - b.length)[0];
	const seen = new Set([first]);
	for (let edge = [first]; edge.length; ) {
		edge = edge.flatMap((url) => links(url).map((a) => a.href)).filter((url) => all.has(url) && !seen.has(url));
		for (const url of edge) seen.add(url);
	}
	expect([...seen].sort()).toEqual([...all].sort());
});
