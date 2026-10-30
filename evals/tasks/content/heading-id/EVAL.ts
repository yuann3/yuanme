import { expect, test } from 'vitest';
import { main, page, text } from '../../../lib/grade.ts';

const headings = (body = main(page('/blog/hello/'))) =>
	body.querySelectorAll('h2, h3').filter((h) => text(h).replace(/[‘’]/g, "'") === "Gauss's theorem");

test('the section keeps its new name', () => {
	expect(headings()).toHaveLength(1);
});

test('#a-theorem leads to it', () => {
	const body = main(page('/blog/hello/'));
	const [h] = headings(body);
	const target = body.querySelector('[id="a-theorem"]');
	expect(target, 'an element has the id a-theorem').not.toBeNull();
	// The heading, something in it, or an anchor just before it.
	const before = target?.nextElementSibling === h || target?.parentNode?.nextElementSibling === h;
	expect(target === h || h.querySelector('[id="a-theorem"]') !== null || before).toBe(true);
});
