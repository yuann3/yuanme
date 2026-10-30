// The parity test's pixel and behaviour checks (#23), the half that needs a
// browser. `cargo run -p parity` runs the URL and DOM half.
//
//   node parity.ts check BASELINE SITE [--engines chromium,firefox,webkit] [--only pixels|feel] [--out DIR]
//   node parity.ts self-test BASELINE [--out DIR]
//
// BASELINE is an unpacked Parity baseline (../fetch.sh): its site/ and the
// remote/ snapshot that answers every other origin. Exits 1 on a difference.
import { appendFileSync, cpSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { parseArgs } from 'node:util';
import { chromium, firefox, webkit, type Browser } from 'playwright';
import { checkRules, pages, serve, type Server } from './lib.ts';
import { pixels, RULES as PIXEL_RULES, type Side } from './pixels.ts';
import { compareFeel, feel, RULES as FEEL_RULES } from './feel.ts';

const ENGINES = { chromium, firefox, webkit };

const { positionals, values } = parseArgs({
	allowPositionals: true,
	options: { engines: { type: 'string', default: 'chromium' }, only: { type: 'string' }, out: { type: 'string', default: 'parity-out' } },
});
const [command, baselineDir, siteDir] = positionals;
checkRules([...PIXEL_RULES, ...FEEL_RULES]);
const remote = join(baselineDir ?? '', 'remote');
if (!baselineDir || !existsSync(join(baselineDir, 'site')) || !existsSync(join(remote, 'index.json'))) {
	console.error('usage: node parity.ts check BASELINE SITE [--engines …] [--only pixels|feel] [--out DIR] | self-test BASELINE');
	process.exit(2);
}
if (values.only !== undefined && values.only !== 'pixels' && values.only !== 'feel') {
	console.error(`--only takes pixels or feel, not ${values.only}`);
	process.exit(2);
}

/** Pixels and behaviour of one candidate in one engine; prints and returns what failed. */
async function check(browser: Browser, engine: string, baseline: Side & { feel?: Record<string, unknown> }, candidate: Server, paths: string[], only?: 'pixels' | 'feel') {
	const failures: string[] = [];
	if (only !== 'feel') {
		for (const f of await pixels(browser, engine, baseline, { server: candidate, remote }, paths, values.out!)) {
			failures.push(`pixels ${engine} ${f.viewport} ${f.page}${f.reason.startsWith(':') ? '' : ': '}${f.reason}`);
		}
	}
	if (only !== 'pixels') {
		baseline.feel ??= await feel(browser, baseline.server, remote);
		const line = (d: ReturnType<typeof compareFeel>[number]) => `behaviour ${engine} ${d.check}: baseline ${JSON.stringify(d.baseline)}, candidate ${JSON.stringify(d.candidate)}`;
		const first = compareFeel(baseline.feel, await feel(browser, candidate, remote));
		for (const d of first.filter((d) => d.tolerated)) console.log(`tolerated by \`${d.tolerated}\`: ${line(d)}`);
		const differing = first.filter((d) => !d.tolerated);
		// The checks race the Router (two clicks 5 ms apart, fixed waits), so
		// on a busy runner one can come out differently. A difference counts
		// only if both sides, run again, still differ there.
		if (differing.length) {
			const again = compareFeel(await feel(browser, baseline.server, remote), await feel(browser, candidate, remote));
			for (const d of differing) {
				if (again.some((a) => a.check === d.check)) failures.push(line(d));
				else console.log(`differed once, then matched when run again: ${line(d)}`);
			}
		}
	}
	return failures;
}

type Edit = ['replace', string, string, string] | ['append', string, string];
type Seed = { name: string; edits: Edit[]; pages?: string[]; only: 'pixels' | 'feel'; caught: boolean };

/** The stylesheet every Page links to. The DOM check doesn't read CSS, so a change here is the pixel check's alone. */
const CSS = '_astro/about.DhiEpsgV.css';
/**
 * Changes seeded into a copy of the baseline, each checked on the Pages it
 * touches: those the test must catch, and those it must let through.
 */
const SEEDS: Seed[] = [
	{ name: 'a colour changed', only: 'pixels', pages: ['/', '/blog/'], caught: true, edits: [['append', CSS, '.headline{color:#c00}']] },
	{ name: 'a heading moved 2px', only: 'pixels', pages: ['/about/'], caught: true, edits: [['append', CSS, '.panel-title{margin-top:2px}']] },
	{ name: "the Film's fallback brighter", only: 'pixels', pages: ['/about/'], caught: true, edits: [['append', CSS, '.film-fallback{filter:brightness(1.2)}']] },
	{ name: 'markup rewritten to the same DOM', only: 'pixels', pages: ['/'], caught: false, edits: [['replace', 'index.html', '<head>', '<head>\n  <!-- rewritten -->\n  '], ['replace', 'index.html', 'class="nav-link" data-nav-link', 'data-nav-link="" class="nav-link"']] },
	// The same change as the Film's, inside the photos: within the photo threshold.
	{ name: 'About photos re-encoded (about-image-bytes)', only: 'pixels', pages: ['/about/'], caught: false, edits: [['append', CSS, '[data-pile] img{filter:brightness(1.2)}']] },
	// Two of the tags index's rows swapped: the Engine's order, not Astro's.
	{ name: 'tags in another order (collection-order)', only: 'pixels', pages: ['/tags/'], caught: false, edits: [['replace', 'tags/index.html', '>#life<', '>#swap<'], ['replace', 'tags/index.html', '>#c<', '>#life<'], ['replace', 'tags/index.html', '>#swap<', '>#c<']] },
	{ name: 'PhotoPile broken', only: 'feel', caught: true, edits: [['replace', '_astro/PhotoPile.astro_astro_type_script_index_0_lang.Cmp7FTzI.js', 'dataset.init', 'dataset.unused']] },
	{ name: 'no Router: full page loads', only: 'feel', caught: true, edits: [['replace', '*', '<script type="module" src="/_astro/ClientRouter.astro_astro_type_script_index_0_lang.C8icoCZT.js"></script>', '']] },
];

function htmlFiles(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		return statSync(path).isDirectory() ? htmlFiles(path) : name.endsWith('.html') ? [path] : [];
	});
}

function apply(site: string, edits: Edit[]) {
	for (const edit of edits) {
		if (edit[0] === 'append') {
			appendFileSync(join(site, edit[1]), edit[2]);
			continue;
		}
		const [, file, from, to] = edit;
		let found = false;
		for (const path of file === '*' ? htmlFiles(site) : [join(site, file)]) {
			const text = readFileSync(path, 'utf8');
			found ||= text.includes(from);
			writeFileSync(path, text.replaceAll(from, to));
		}
		if (!found) throw new Error(`seed edit ${JSON.stringify(edit)} no longer applies`);
	}
}

const engines = values.engines!.split(',') as (keyof typeof ENGINES)[];
const baseline = await serve(join(baselineDir, 'site'));
let ok = true;
for (const engine of engines) {
	// Grayscale text antialiasing, as on macOS: on Linux, Chromium's LCD
	// antialiasing depends on whether a text run has an opaque background
	// of its own, which isn't what the pixel diff is for.
	const browser = await ENGINES[engine].launch(engine === 'chromium' ? { args: ['--disable-lcd-text'] } : {});
	console.log(`${engine} ${browser.version()}`);
	const side = { server: baseline, remote, cache: new Map() };
	if (command === 'check' && siteDir) {
		const candidate = await serve(siteDir);
		const paths = pages(join(baselineDir, 'site')).filter((p) => existsSync(join(siteDir, p.endsWith('/') ? p + 'index.html' : p)));
		const failures = await check(browser, engine, side, candidate, paths, values.only as 'pixels' | 'feel' | undefined);
		failures.forEach((f) => console.log(f));
		console.log(`${values.only ?? 'pixels and behaviour'}, ${engine}: ${failures.length} differences from the Parity baseline`);
		ok &&= failures.length === 0;
		await candidate.close();
	} else if (command === 'self-test') {
		// Against itself: a second render of the same Site, so this is also
		// the test that rendering is deterministic.
		const again = await serve(join(baselineDir, 'site'));
		const failures = await check(browser, engine, side, again, pages(join(baselineDir, 'site')));
		failures.forEach((f) => console.log(f));
		console.log(`${failures.length ? 'FAIL' : 'ok  '} the baseline against itself`);
		ok &&= failures.length === 0;
		await again.close();
		for (const seed of SEEDS) {
			const dir = mkdtempSync(join(tmpdir(), 'parity-seed-'));
			cpSync(join(baselineDir, 'site'), dir, { recursive: true });
			apply(dir, seed.edits);
			const server = await serve(dir);
			const failures = await check(browser, engine, side, server, seed.pages ?? [], seed.only);
			const pass = seed.caught === failures.length > 0;
			console.log(`${pass ? 'ok  ' : 'FAIL'} ${seed.name}: ${failures.length ? `${failures.length} differences` : 'no differences'}`);
			for (const f of failures) console.log(`       ${f.slice(0, 200)}`);
			ok &&= pass;
			await server.close();
			rmSync(dir, { recursive: true });
		}
	} else {
		console.error(`unknown command ${command}`);
		process.exit(2);
	}
	await browser.close();
}
await baseline.close();
process.exit(ok ? 0 : 1);
