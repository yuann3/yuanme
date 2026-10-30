import { expect, test } from 'vitest';
import { page, reachable, routes, text } from '../../../lib/grade.ts';

/** Each Theorem block about primes on a Page the home Page leads to. */
const theorems = () =>
	[...reachable(2)].flatMap((url) =>
		page(url)
			.querySelectorAll('.theorem')
			.filter((block) => /prime/i.test(text(block)))
			.map((block) => ({ url, block })),
	);

test('a Post the home Page leads to states a numbered theorem about primes, and proves it', () => {
	expect(theorems().map((t) => t.url)).not.toEqual([]);
	for (const { url, block } of theorems()) {
		expect(text(block.querySelector('.theorem-name') ?? block), url).toMatch(/^Theorem \d+$/);
		expect(block.nextElementSibling?.classList.contains('proof'), `${url} has a proof right after the theorem`).toBe(true);
	}
});

test("it is the blog's first Post", () => {
	const date = (url: string) => page(url).querySelector('time[datetime]')?.getAttribute('datetime');
	for (const { url } of theorems()) {
		const post = routes().find((r) => r.url === url && r.kind === 'entry');
		const first = date(url);
		expect(post, `${url} is a Post`).toBeDefined();
		if (!first) continue;
		for (const other of routes().filter((r) => r.kind === 'entry' && r.collection === post!.collection)) {
			const d = date(other.url);
			if (d) expect(d >= first, `${other.url} is from ${d}, before ${url}`).toBe(true);
		}
	}
});
