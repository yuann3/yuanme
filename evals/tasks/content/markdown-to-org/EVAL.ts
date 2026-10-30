import { expect, test } from 'vitest';
import { links, main, page, routes, text } from '../../../lib/grade.ts';

test('the Hello Post is written in Org, at its URL', () => {
	const hello = routes().filter((r) => r.kind === 'entry' && r.url === '/blog/hello/');
	expect(hello.map((r) => r.entry)).toEqual([expect.stringMatching(/\.org$/)]);
	expect(text(page('/blog/hello/').querySelector('h1')!)).toBe('Hello');
});

test('no Post is still the Markdown Hello', () => {
	expect(routes().filter((r) => r.entry === 'content/posts/hello.md')).toEqual([]);
});

test('it keeps its equation, theorem, proof, references and code', () => {
	const body = main(page('/blog/hello/'));
	expect(body.querySelectorAll('math[display="block"] [id]').map((el) => el.id)).toContain('eq:odd');
	expect(text(body.querySelector('.theorem')!)).toMatch(/^Theorem 1 \(Gauss\)/);
	expect(body.querySelector('.proof')).not.toBeNull();
	expect(links('/blog/hello/').filter((a) => a.hash === 'eq:odd').map((a) => a.text)).toContain('(1)');
	expect(text(body.querySelector('pre[data-language="rust"]')!)).toContain('fn odd_sum');
});
