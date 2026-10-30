import { expect, test } from 'vitest';
import { links, main, page, pages, text } from '../../../lib/grade.ts';

test("the home Page is Ada Lovelace's", () => {
	expect(text(page('/'))).toContain('Ada Lovelace');
});

test('the home Page links to a blog', () => {
	const blog = links('/').filter((a) => /blog/i.test(a.text) || /blog/i.test(a.href));
	expect(blog.length).toBeGreaterThan(0);
	for (const a of blog) page(a.href);
});

test('a Page reached from the home Page lists the two projects and nothing else', () => {
	const reached = new Set(['/', ...links('/').map((a) => a.href)]);
	const listing = pages().filter(
		({ url, html }) => reached.has(url) && /Analytical Engine/.test(text(main(html))) && /Bernoulli/.test(text(main(html))),
	);
	expect(listing.map((p) => p.url)).not.toEqual([]);
	for (const { html } of listing) expect(text(main(html))).not.toContain('Example project');
});
