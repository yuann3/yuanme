import { expect, test } from 'vitest';
import { page, routes, text } from '../../../lib/grade.ts';

const post = () =>
	routes().find((r) => r.kind === 'entry' && /^Why I write in Org$/.test(text(page(r.url).querySelector('h1') ?? page(r.url))));

test('there is a Post called "Why I write in Org", written in Org', () => {
	expect(post()?.entry).toMatch(/\.org$/);
});

test('every Post of its blog is written in Org', () => {
	const blog = post()?.collection;
	const entries = routes().filter((r) => r.kind === 'entry' && r.collection === blog);
	expect(entries.filter((r) => !r.entry?.endsWith('.org')).map((r) => r.entry)).toEqual([]);
	expect(routes().some((r) => r.kind === 'list' && r.collection === blog)).toBe(true);
});
