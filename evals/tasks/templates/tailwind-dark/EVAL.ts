import { expect, test } from 'vitest';
import { output, page } from '../../../lib/grade.ts';

/** The stylesheets the home Page links, by URL. */
const sheets = () =>
	page('/')
		.querySelectorAll('link[rel="stylesheet"]')
		.map((l) => output(new URL(l.getAttribute('href')!, 'https://site.invalid/').pathname) ?? '');

/** The classes the home Page uses. */
const classes = () => [...new Set(page('/').querySelectorAll('[class]').flatMap((el) => el.classList.value))];

const escape = (name: string) => name.replace(/[^a-zA-Z0-9_-]/g, (c) => `\\${c}`);

test('the home Page is styled with Tailwind', () => {
	const tailwind = sheets().filter((css) => /tailwindcss v4/.test(css));
	expect(tailwind, 'a stylesheet built by Tailwind').toHaveLength(1);
	const used = classes().filter((name) => tailwind[0].includes(`.${escape(name)}`));
	expect(used.length, `Tailwind classes on the home Page: ${used.join(' ')}`).toBeGreaterThanOrEqual(5);
});

test('its dark mode follows the system setting', () => {
	// Tailwind's dark: variant, or rules of the Author's own.
	const tailwind = sheets().find((css) => /tailwindcss v4/.test(css)) ?? '';
	expect(tailwind).toMatch(/@media\s*\(prefers-color-scheme:\s*dark\)\s*\{[^}]/);
});
