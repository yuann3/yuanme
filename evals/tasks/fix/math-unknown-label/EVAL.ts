import { expect, test } from 'vitest';
import { links, main, page } from '../../../lib/grade.ts';

test("the theorem refers to the odd-numbers equation by its number", () => {
	const theorem = main(page('/blog/hello/')).querySelector('.theorem');
	expect(theorem).not.toBeNull();
	const refs = links(theorem!, '/blog/hello/').filter((a) => a.href === '/blog/hello/' && a.hash === 'eq:odd');
	expect(refs.map((a) => a.text)).toContain('(1)');
});
