import { statSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { build, main, page } from '../../../lib/grade.ts';

/** The hero image on the home Page. */
const hero = () => main(page('/')).querySelectorAll('img').filter((img) => /hills at sunrise/i.test(img.getAttribute('alt') ?? ''));

/** The bytes of a file of the Site, by its URL from the home Page. */
const bytes = (url: string) => statSync(join(build().out, new URL(url, 'https://site.invalid/').pathname)).size;

/**
 * What a phone fetches for a srcset: the smallest file at least 800 pixels
 * wide, which fills a 400-pixel-wide screen at 2x, or else the widest.
 */
function phone(srcset: string): string {
	const candidates = srcset.split(',').map((c) => {
		const [url, width] = c.trim().split(/\s+/);
		return { url, width: Number.parseInt(width ?? '0') };
	});
	const fits = candidates.filter((c) => c.width >= 800).sort((a, b) => a.width - b.width);
	return (fits[0] ?? candidates.sort((a, b) => b.width - a.width)[0]).url;
}

test('the home Page still shows the hero image', () => {
	expect(hero()).toHaveLength(1);
});

test('a phone downloads at most 200 kB for it', () => {
	const img = hero()[0];
	const sets = [...(img.parentNode?.tagName === 'PICTURE' ? img.parentNode.querySelectorAll('source') : []), img]
		.map((el) => el.getAttribute('srcset'))
		.filter((s): s is string => Boolean(s));
	const fetched = sets.length ? phone(sets[0]) : img.getAttribute('src')!;
	expect(bytes(fetched), fetched).toBeLessThanOrEqual(200_000);
	expect(bytes(img.getAttribute('src')!), 'the fallback src').toBeLessThanOrEqual(400_000);
});
