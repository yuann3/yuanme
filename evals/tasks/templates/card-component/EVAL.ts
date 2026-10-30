import { expect, test } from 'vitest';
import { links, routes, source } from '../../../lib/grade.ts';

const STARTER = ['post-row', 'pagination', 'tag-list', 'code-copy'];

test('the home Page links to every project', () => {
	const projects = routes().filter((r) => r.kind === 'entry' && r.collection === 'projects');
	expect(projects.length).toBeGreaterThan(1);
	const linked = links('/').map((a) => a.href);
	for (const p of projects) expect(linked).toContain(p.url);
});

test('the home Page draws them with a Component of its own', () => {
	const home = routes().find((r) => r.url === '/')!.template!;
	const imported = [...source(`templates/${home}`).matchAll(/\{%-?\s*(?:from|import)\s+"(components\/[^"]+)"/g)].map((m) => m[1]);
	const cards = imported.filter((path) => !STARTER.some((name) => path === `components/${name}.html`));
	expect(cards, 'a Component the Starter has not').not.toEqual([]);
	for (const path of cards) expect(source(`templates/${path}`)).toMatch(/\{#\s*props:/);
});
