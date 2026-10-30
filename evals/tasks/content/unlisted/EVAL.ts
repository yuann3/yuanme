import { expect, test } from 'vitest';
import { links, output, page, pages, text } from '../../../lib/grade.ts';

const url = '/blog/hello-org/';

test('the Post is still at its URL', () => {
	expect(text(page(url).querySelector('h1')!)).toBe('Hello from Org');
});

test('no other Page links to it', () => {
	const linking = pages().filter((p) => p.url !== url && links(p.html, p.url).some((a) => a.href === url));
	expect(linking.map((p) => p.url)).toEqual([]);
});

test('the feed leaves it out', () => {
	expect(output('/rss.xml')).toBeDefined();
	expect(output('/rss.xml')).not.toContain(url);
});
