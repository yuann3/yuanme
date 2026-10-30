import { expect, test } from 'vitest';
import { links, main, page, text } from '../../../lib/grade.ts';

test("the theorem has a footnote that cites the Disquisitiones", () => {
	const body = main(page('/blog/hello/'));
	const theorem = body.querySelector('.theorem');
	expect(theorem).not.toBeNull();
	const notes = links(theorem!, '/blog/hello/')
		.filter((a) => a.href === '/blog/hello/' && a.hash)
		.map((a) => body.querySelector(`[id="${a.hash}"]`))
		.filter((note) => note && /Disquisitiones/.test(text(note)));
	expect(notes.length).toBeGreaterThan(0);
	// The note is apart from the theorem's own text.
	expect(text(theorem!)).not.toMatch(/Disquisitiones/);
});
