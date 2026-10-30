import { expect, test } from 'vitest';
import { pages, routes, source, text } from '../../../lib/grade.ts';

const galois = (drafts: boolean) =>
	routes(drafts).filter((r) => r.kind === 'entry' && /Notes on Galois theory/.test(source(r.entry!)));

test('a preview of the Site has the Post', () => {
	expect(galois(true).map((r) => r.url)).toHaveLength(1);
});

test('the published Site has no trace of it', () => {
	expect(galois(false)).toEqual([]);
	expect(pages().filter((p) => /Galois/.test(text(p.html))).map((p) => p.url)).toEqual([]);
});
