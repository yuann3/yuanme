// Byte sizes: raw (minified) / gzip -9 / brotli -q 11.
// Usage: bun bench/sizes.ts <eyuan.me clone on proto/shell-router> <out dir with dists>
import { build } from 'esbuild';
import { cpSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync, existsSync } from 'node:fs';
import { join } from 'node:path';

const [site, out] = process.argv.slice(2);
const tmp = join(import.meta.dir, '../tmp-sizes');
rmSync(tmp, { recursive: true, force: true });
mkdirSync(tmp, { recursive: true });

const sh = (cmd: string[], input: Uint8Array) => Bun.spawnSync(cmd, { stdin: input }).stdout.length;
const measure = (buf: Uint8Array | string) => {
	const b = typeof buf === 'string' ? new TextEncoder().encode(buf) : buf;
	return { raw: b.length, gz: sh(['gzip', '-9c'], b), br: sh(['brotli', '-q', '11', '-c'], b) };
};
const fmt = (m: { raw: number, gz: number, br: number }) => `${m.raw} / ${m.gz} / ${m.br}`;
const rows: [string, ReturnType<typeof measure>][] = [];

async function bundle(entry: string, external: string[] = []) {
	const r = await build({ entryPoints: [entry], bundle: true, minify: true, format: 'esm', write: false, external, target: 'es2022', logLevel: 'error' });
	return r.outputFiles[0].text;
}

// 1. Router source (esbuild, the #12 method)
const src = join(tmp, 'src');
cpSync(join(site, 'src/scripts'), src, { recursive: true });
const router = await bundle(join(src, 'router.ts'));
rows.push(['router.ts alone (Navigation API driver + swap core + scripts + prefetch + announcer + dev hook)', measure(router)]);
writeFileSync(join(tmp, 'router.min.js'), router);

// without the dev hook (what a production page needs)
const noDev = readFileSync(join(src, 'router.ts'), 'utf8').replace(/\/\/ Dev soft-swap hook[\s\S]*$/, '');
writeFileSync(join(src, 'router-nodev.ts'), noDev);
const routerNoDev = await bundle(join(src, 'router-nodev.ts'));
rows.push(['router.ts without the dev hook', measure(routerNoDev)]);

// without prefetch
const noPf = noDev.replace(/\/\/ Hover\/focus prefetch[\s\S]*?\n}\n/, '');
writeFileSync(join(src, 'router-nopf.ts'), noPf);
rows.push(['router.ts without dev hook and prefetch', measure(await bundle(join(src, 'router-nopf.ts')))]);

// History API driver: router + history vs router
writeFileSync(join(src, 'both.ts'), "import './router';\nimport './router-history';\n");
const both = await bundle(join(src, 'both.ts'));
rows.push(['router.ts + router-history.ts (History API driver)', measure(both)]);

// 2. Shell glue: shell.ts with and without the swap integration, router external
const shell = readFileSync(join(src, 'shell.ts'), 'utf8');
const cut = (s: string, from: string, to: string) => s.slice(0, s.indexOf(from)) + s.slice(s.indexOf(to));
let bare = cut(shell, '// ---------- panel morph', '// ---------- nav ----------');
bare = cut(bare, 'function syncNav', 'let previewing');
bare = cut(bare, '// ---------- the swap ----------', '// ---------- first load ----------');
bare += '\nexport { preview, hintOn, hintOut, introItems, play, unobserve, enter, HINT_DELAY_MS, COLLAPSE_MS, EXPAND_MS, EXIT_MS, RADIUS, EASE_IN_OUT };\n';
writeFileSync(join(src, 'shell-bare.ts'), bare);
const ext = [join(src, 'router.ts'), './router'];
const shellFull = await bundle(join(src, 'shell.ts'), ['./router']);
const shellBare = await bundle(join(src, 'shell-bare.ts'), ['./router']);
rows.push(['shell.ts bundle (film, palettes, reveal; router external)', measure(shellFull)]);
rows.push(['shell.ts bundle without glue (morph, syncNav, swapPanel, before-swap)', measure(shellBare)]);
const devless = shell.replace(/\t\/\/ Dev soft swap \(#11\)[\s\S]*?\t\treturn;\n\t}\n\n/, '');
writeFileSync(join(src, 'shell-nodev.ts'), devless);
rows.push(['shell.ts bundle without the dev branch of the glue', measure(await bundle(join(src, 'shell-nodev.ts'), ['./router']))]);
// glue alone, as its own module: the cut parts plus imports
const glueOnly = `import { navigate, swapHeadElements, swapRootAttributes } from './router';\nimport { enter, play, unobserve } from './motion-reveal';\n` +
	shell.slice(shell.indexOf('type State'), shell.indexOf('// ---------- nav ----------')) +
	shell.slice(shell.indexOf('function syncNav'), shell.indexOf('let previewing')) +
	'declare const preview: any, hintOn: any, hintOut: any, introItems: any, HINT_DELAY_MS: number;\n' +
	shell.slice(shell.indexOf('// ---------- the swap ----------'), shell.indexOf('// ---------- first load ----------'));
writeFileSync(join(src, 'glue-only.ts'), glueOnly.replace(/^const reduceQuery[\s\S]*?const state = [^\n]*\n/m, (m) => m));
rows.push(['glue alone (morph + syncNav + swapPanel + before-swap, motion-reveal external)', measure(await bundle(join(src, 'glue-only.ts'), ['./router', './motion-reveal']))]);

// 3. Dist: chunks and per-page totals (external module graph + inline scripts)
function pageJs(dist: string, page: string) {
	const html = readFileSync(join(dist, page), 'utf8');
	const files = new Set<string>();
	const visit = (p: string) => {
		if (files.has(p)) return;
		files.add(p);
		const js = readFileSync(join(dist, p), 'utf8');
		for (const m of js.matchAll(/(?:import|from)\s*["'](\.\/[^"']+\.js)["']/g)) visit(join('_astro', m[1].slice(2)));
	};
	for (const m of html.matchAll(/<script[^>]*src="\/(_astro\/[^"]+\.js)"/g)) visit(m[1]);
	const inline = [...html.matchAll(/<script(?![^>]*src=)[^>]*>([\s\S]*?)<\/script>/g)].map((m) => m[1]).filter((s) => s.trim());
	const ext = [...files].map((f) => readFileSync(join(dist, f)));
	const sum = (ms: ReturnType<typeof measure>[]) => ms.reduce((a, m) => ({ raw: a.raw + m.raw, gz: a.gz + m.gz, br: a.br + m.br }), { raw: 0, gz: 0, br: 0 });
	return { files: [...files], requests: files.size, ext: sum(ext.map(measure)), inline: sum(inline.map(measure)), all: sum([...ext, ...inline.map((s) => new TextEncoder().encode(s))].map(measure)) };
}
const dists = ['astro-dist', 'proto-dist', 'proto-hist-dist'];
const lines: string[] = [];
for (const d of dists) {
	const dist = join(out, d);
	if (!existsSync(dist)) continue;
	for (const f of readdirSync(join(dist, '_astro')).filter((f) => f.endsWith('.js'))) rows.push([`${d}/_astro/${f}`, measure(readFileSync(join(dist, '_astro', f)))]);
	for (const p of ['index.html', 'blog/index.html', 'blog/extending-c-stdlib/index.html', 'about/index.html']) {
		const r = pageJs(dist, p);
		lines.push(`${d} ${p}: JS requests ${r.requests}; external ${fmt(r.ext)}; inline ${fmt(r.inline)}; total ${fmt(r.all)}`);
	}
}

const table = rows.map(([k, m]) => `| ${k} | ${fmt(m)} |`).join('\n');
const report = `| item | raw / gzip -9 / brotli q11 (B) |\n|---|---|\n${table}\n\nPer-page first-party JS (each file compressed separately, as served):\n${lines.join('\n')}\n`;
console.log(report);
mkdirSync(join(import.meta.dir, '../../results'), { recursive: true });
writeFileSync(join(import.meta.dir, '../../results/sizes.md'), report);
writeFileSync(join(import.meta.dir, '../../results/sizes.json'), JSON.stringify({ rows, lines }, null, 1));
