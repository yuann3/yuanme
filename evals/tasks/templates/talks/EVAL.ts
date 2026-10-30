import { expect, test } from 'vitest';
import { main, pages, text } from '../../../lib/grade.ts';

const TALKS = ['Org for mathematicians', 'Proofs as programs', 'Typed templates'];

const listing = () => pages().filter(({ html }) => TALKS.every((t) => text(main(html)).includes(t)));

test('a Page lists the three talks, with where each was given', () => {
	expect(listing().map((p) => p.url)).toHaveLength(1);
	const shown = text(main(listing()[0].html));
	for (const event of ['Strange Loop', 'RustConf', 'EmacsConf']) expect(shown).toContain(event);
});

test('newest first', () => {
	const shown = text(main(listing()[0].html));
	const at = TALKS.map((t) => shown.indexOf(t));
	expect(at).toEqual([...at].sort((a, b) => a - b));
});
