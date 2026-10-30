import { expect, test } from 'vitest';
import { links, main, page, routes, text } from '../../../lib/grade.ts';

const post = () => {
	const found = routes().filter((r) => r.kind === 'entry' && /quadratic/i.test(text(page(r.url).querySelector('h1') ?? page(r.url))));
	expect(found.map((r) => r.url)).toHaveLength(1);
	return found[0].url;
};

/** The element that shows equation number `n`, as the Engine writes it. */
const number = (url: string, n: number) =>
	main(page(url))
		.querySelectorAll('math[display="block"] [id]')
		.find((el) => text(el) === `(${n})`);

test('the Post numbers its equations', () => {
	const body = main(page(post()));
	expect(body.querySelectorAll('math[display="block"]').length).toBeGreaterThan(0);
	const shown = body.querySelectorAll('math[display="block"] mtext').map(text);
	expect(shown).toContain('(1)');
	expect(shown).toContain('(2)');
});

test('it refers back to equation (1) by its number', () => {
	const first = number(post(), 1);
	expect(first, 'equation (1) has an id to link to').toBeDefined();
	const refs = links(post()).filter((a) => a.hash === first!.id && a.href === post());
	expect(refs.map((a) => a.text)).toContain('(1)');
});
