import { expect, test } from 'vitest';
import { main, output, page, text } from '../../../lib/grade.ts';

const POSTS = {
	'/blog/first-light/': ['First light', 'somewhere to put the notes I keep losing'],
	'/blog/on-recursion/': ['On recursion', 'every call makes some measure smaller'],
	'/blog/2024/recap/': ['2024 in review', 'a parser combinator library'],
};

test('each Post of the Astro blog is at its URL, with its text', () => {
	for (const [url, [title, words]] of Object.entries(POSTS)) {
		const body = main(page(url));
		expect(text(body.querySelector('h1') ?? body), url).toBe(title);
		expect(text(body), url).toContain(words);
	}
});

test('so are the home Page, the blog list and the about Page', () => {
	expect(text(main(page('/blog/')))).toContain('On recursion');
	expect(text(main(page('/about/')))).toContain('small languages and long walks');
	page('/');
});

test('the feed is at /rss.xml and has every Post', () => {
	const feed = output('/rss.xml') ?? '';
	for (const url of Object.keys(POSTS)) expect(feed).toContain(`https://example.com${url}`);
});
