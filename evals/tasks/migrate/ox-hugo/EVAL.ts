import { expect, test } from 'vitest';
import { main, output, page, text } from '../../../lib/grade.ts';

test('each finished Post is at the URL Hugo gave it', () => {
	expect(text(page('/posts/hello-hugo/').querySelector('h1')!)).toBe('Hello, Hugo');
	expect(text(page('/posts/fixed-points/').querySelector('h1')!)).toBe('Fixed points');
});

test('with its math and code', () => {
	const body = main(page('/posts/fixed-points/'));
	expect(body.querySelectorAll('math').length).toBeGreaterThan(1);
	expect(body.querySelectorAll('math[display="block"] mtext').map(text)).toContain('(1)');
	expect(text(body.querySelector('pre[data-language="python"]')!)).toContain('def fixed_point');
});

test('the TODO Post, a draft to ox-hugo, is not published', () => {
	expect(output('/posts/monads/')).toBeUndefined();
});
