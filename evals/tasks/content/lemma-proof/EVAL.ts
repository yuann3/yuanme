import { expect, test } from 'vitest';
import { main, page, text } from '../../../lib/grade.ts';

const lemmas = () =>
	main(page('/blog/hello/'))
		.querySelectorAll('.theorem[data-kind="lemma"]')
		.filter((block) => /odd/i.test(text(block)) && /even/i.test(text(block)));

test('the Hello Post has a lemma about the sum of two odd numbers', () => {
	expect(lemmas()).toHaveLength(1);
	expect(text(lemmas()[0])).toMatch(/^Lemma \d+/);
});

test('a proof follows it', () => {
	expect(lemmas()[0].nextElementSibling?.classList.contains('proof')).toBe(true);
});

test('the Post keeps its theorem', () => {
	expect(main(page('/blog/hello/')).querySelector('.theorem[data-kind="theorem"]')).not.toBeNull();
});
