import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { expect, test } from 'vitest';
import { build } from '../../../lib/grade.ts';

/** Every file of the built Site, as text. */
function everything(): string {
	const files: string[] = [];
	const walk = (dir: string) => {
		for (const name of readdirSync(dir)) {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) walk(path);
			else if (/\.(html|js|json|txt)$/.test(name)) files.push(readFileSync(path, 'utf8'));
		}
	};
	walk(build().out);
	return files.join('\n');
}

// The build's JS budget is the "slow on phones" part: the invariant that
// the Site builds with --strict-budgets checks it.
test('the Site still has every quote', () => {
	const site = everything();
	expect(site).toContain('Quote 1: ');
	expect(site).toContain('Quote 1500: ');
});
