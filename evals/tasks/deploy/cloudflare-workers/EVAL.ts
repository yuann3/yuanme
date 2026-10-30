import { existsSync, readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
import { output } from '../../../lib/grade.ts';

test('a Wrangler config serves the built Site', () => {
	const file = ['wrangler.jsonc', 'wrangler.json', 'wrangler.toml'].find((f) => existsSync(f));
	expect(file, 'a wrangler.jsonc, .json or .toml').toBeDefined();
	// yuanme check reports a wrangler.jsonc that serves another folder;
	// this reads the other two.
	expect(readFileSync(file!, 'utf8')).toMatch(/directory\s*["=:]+\s*"(\.\/)?dist\/?"/);
});

test('browsers are told to use only HTTPS for at least a year', () => {
	const all = (output('/_headers') ?? '').split(/\n(?=\/)/).find((block) => block.startsWith('/*\n')) ?? '';
	const maxAge = /Strict-Transport-Security:\s*max-age=(\d+)/i.exec(all)?.[1];
	expect(Number(maxAge)).toBeGreaterThanOrEqual(31_536_000);
});

test('/feed.xml redirects to /rss.xml', () => {
	const rules = (output('/_redirects') ?? '').split('\n').map((line) => line.trim().split(/\s+/));
	expect(rules.some(([from, to, status]) => from === '/feed.xml' && to === '/rss.xml' && /^30[1278]$/.test(status ?? '301'))).toBe(true);
});
