import { expect, test } from 'vitest';
import { main, page, text } from '../../../lib/grade.ts';

test("each Post shows its description under its title", () => {
	expect(text(main(page('/blog/hello/')))).toContain('A first Post, with an equation, a theorem and some code.');
	expect(text(main(page('/blog/hello-org/')))).toContain('The same ideas as the first Post, written in Org.');
});
