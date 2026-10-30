import { expect, test } from 'vitest';
import { links, routes } from '../../../lib/grade.ts';

test('the Post keeps the tag it was given', () => {
	const meta = routes().find((r) => r.kind === 'term' && r.taxonomy === 'tags' && r.term === 'meta');
	expect(meta).toBeDefined();
	expect(links(meta!.url).map((a) => a.href)).toContain('/blog/hello/');
});
