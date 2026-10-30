import { expect, test } from 'vitest';
import { main, page, reachable, text } from '../../../lib/grade.ts';

const resume = () =>
	[...reachable(1)].filter((url) => url !== '/' && /Eckert.Mauchly/.test(text(main(page(url)))));

test('a Page linked from the home Page holds the CV', () => {
	expect(resume()).toHaveLength(1);
	const cv = text(main(page(resume()[0])));
	for (const part of ['Grace Hopper', 'grace@example.com', 'Navy Programming Languages Group', 'Harvard Computation Laboratory', 'Yale', 'Vassar', 'COBOL', 'FLOW-MATIC']) {
		expect(cv).toContain(part);
	}
});

test('its jobs keep the CV\'s order, latest first', () => {
	const cv = text(main(page(resume()[0])));
	const at = ['Navy Programming Languages Group', 'Eckert', 'Harvard Computation Laboratory'].map((job) => cv.indexOf(job));
	expect(at).toEqual([...at].sort((a, b) => a - b));
});
