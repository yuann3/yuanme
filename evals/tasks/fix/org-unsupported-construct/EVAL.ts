import { expect, test } from 'vitest';
import { main, page, text } from '../../../lib/grade.ts';

test('the Org Post still shows the included code', () => {
	const code = main(page('/blog/hello-org/')).querySelectorAll('pre').map(text);
	expect(code.some((c) => c.includes('fn odd_sum(n: u64) -> u64') && c.includes('2 * k - 1'))).toBe(true);
});
