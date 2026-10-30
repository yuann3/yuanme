import { expect, test } from 'vitest';
import { links } from '../../../lib/grade.ts';

test('the about Page links to the first Post', () => {
	const link = links('/about/').filter((a) => a.text === 'my first Post');
	expect(link.map((a) => a.href)).toEqual(['/blog/hello/']);
});
