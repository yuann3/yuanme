import { expect, test } from 'vitest';
import { links, page, routes, text } from '../../../lib/grade.ts';

// A series is a Taxonomy (CONTEXT.md): its Term's Page lists the Posts.
const series = () =>
	routes().filter((r) => r.kind === 'term' && r.taxonomy !== 'tags' && /getting started/i.test(text(page(r.url))));

test('a Taxonomy other than tags has the Term "Getting started"', () => {
	expect(series().map((r) => r.url)).toHaveLength(1);
});

test("the Term's Page lists both Hello Posts", () => {
	const listed = links(series()[0].url).map((a) => a.href);
	expect(listed).toContain('/blog/hello/');
	expect(listed).toContain('/blog/hello-org/');
});
