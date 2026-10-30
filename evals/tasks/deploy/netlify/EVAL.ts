import { existsSync, readFileSync } from 'node:fs';
import { expect, test } from 'vitest';
import { output } from '../../../lib/grade.ts';

// Netlify reads _headers and _redirects in the published folder, and
// netlify.toml at the root of the project.
const toml = () => (existsSync('netlify.toml') ? readFileSync('netlify.toml', 'utf8') : '');

test('/posts/hello/ redirects to /blog/hello/ for good', () => {
	const rules = (output('/_redirects') ?? '').split('\n').map((line) => line.trim().split(/\s+/));
	const inFile = rules.some(([from, to, status]) => from === '/posts/hello/' && to === '/blog/hello/' && ['301', '308', '301!', '308!'].includes(status));
	const inToml = /from\s*=\s*"\/posts\/hello\/"\s*\n\s*to\s*=\s*"\/blog\/hello\/"\s*\n\s*status\s*=\s*30[18]\b/.test(toml());
	expect(inFile || inToml).toBe(true);
});

test('no Page may be framed by another site', () => {
	const headers = output('/_headers') ?? '';
	const all = headers.split(/\n(?=\/)/).find((block) => block.startsWith('/*\n')) ?? '';
	const denies = (text: string) => /X-Frame-Options\s*[:=]\s*"?(DENY|SAMEORIGIN)/i.test(text) || /frame-ancestors\s+('none'|'self')/i.test(text);
	expect(denies(all) || (/for\s*=\s*"\/\*"/.test(toml()) && denies(toml()))).toBe(true);
});

test("_headers is in Netlify's form", () => {
	// Cloudflare's `! Header` detaches a header; Netlify reads it as a header named "!".
	expect(output('/_headers') ?? '').not.toMatch(/^\s+!/m);
});
