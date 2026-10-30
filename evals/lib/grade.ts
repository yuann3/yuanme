// What every Eval task's grader reads (#22): one `yuanme build
// --strict-budgets --json` of the project into a fresh folder, its Pages
// parsed, and the project's files. Importing it adds the checks every
// Eval task makes, so no grader can leave them out.
//
// Graders run from the project root with `yuanme` on PATH: in the sandbox
// after the agent finishes, and in the grader self-test (selftest.ts).
import { execFileSync, spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readdirSync, readFileSync, statSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, join, relative } from 'node:path';
import { parse as parseHtml, type HTMLElement } from 'node-html-parser';
import { expect, test } from 'vitest';

export type Diagnostic = {
	code: string;
	level: 'error' | 'warning';
	message: string;
	rendered: string;
	spans: { file: string; line_start: number }[];
};

export type Build = {
	/** The exit code: 0 built, 1 the Site has errors, 2 a bad command. */
	code: number;
	diagnostics: Diagnostic[];
	/** The output folder, outside the project so no earlier build is in it. */
	out: string;
};

let built: Build | undefined;

export function build(): Build {
	if (built) return built;
	const out = mkdtempSync(join(tmpdir(), 'yuanme-eval-'));
	const run = spawnSync('yuanme', ['build', '--strict-budgets', '--json', '--out', out], {
		encoding: 'utf8',
		maxBuffer: 256 << 20,
	});
	if (run.error) throw run.error;
	const events = run.stdout
		.split('\n')
		.filter((line) => line.startsWith('{'))
		.map((line) => JSON.parse(line));
	built = { code: run.status ?? -1, diagnostics: events.filter((e) => e.type === 'diagnostic'), out };
	// summarize.ts reads this line from the test output: which codes the
	// final build still reports, for the fix rate per code.
	console.log(
		`YUANME_EVAL_FINAL ${JSON.stringify({ code: built.code, codes: built.diagnostics.map((d) => d.code) })}`,
	);
	return built;
}

export type Route = {
	url: string;
	kind: 'entry' | 'list' | 'term' | 'index' | 'template' | 'feed';
	template?: string;
	collection?: string;
	/** An Entry's file, from the project root. */
	entry?: string;
	taxonomy?: string;
	term?: string;
};

const routed = new Map<boolean, Route[]>();

/** Every Route of the Site, as `yuanme routes` lists it, with Drafts if `drafts`. */
export function routes(drafts = false): Route[] {
	if (!routed.has(drafts)) {
		const args = ['routes', '--json', ...(drafts ? ['--drafts'] : [])];
		const lines = execFileSync('yuanme', args, { encoding: 'utf8', maxBuffer: 64 << 20 }).split('\n');
		routed.set(
			drafts,
			lines.filter((line) => line.startsWith('{')).map((line) => JSON.parse(line)).filter((e) => e.type === 'route'),
		);
	}
	return routed.get(drafts)!;
}

// node-html-parser leaves what is in a <pre> unparsed by default.
const parse = (html: string) => parseHtml(html, { blockTextElements: { script: true, noscript: true, style: true } });

function outputPath(url: string): string {
	return join(build().out, url.endsWith('/') ? `${url}index.html` : url);
}

/** The text of a file of the built Site, by its URL, or undefined. */
export function output(url: string): string | undefined {
	const path = outputPath(url);
	return existsSync(path) && statSync(path).isFile() ? readFileSync(path, 'utf8') : undefined;
}

/** A built Page, parsed. A Page that isn't there fails the test. */
export function page(url: string): HTMLElement {
	const html = output(url);
	if (html === undefined) throw new Error(`the built Site has no Page at ${url}`);
	return parse(html);
}

/** Every HTML Page of the built Site, by URL. */
export function pages(): { url: string; html: HTMLElement }[] {
	const out = build().out;
	const found: { url: string; html: HTMLElement }[] = [];
	const walk = (dir: string) => {
		for (const entry of readdirSync(dir, { withFileTypes: true })) {
			const path = join(dir, entry.name);
			if (entry.isDirectory()) walk(path);
			else if (entry.name.endsWith('.html')) {
				const url = `/${relative(out, path)}`.replace(/index\.html$/, '');
				found.push({ url, html: parse(readFileSync(path, 'utf8')) });
			}
		}
	};
	walk(out);
	return found.sort((a, b) => a.url.localeCompare(b.url));
}

export type Link = { href: string; hash: string; text: string; el: HTMLElement };

/**
 * The links of the Page at `url`, or of an element, that lead into the
 * Site: `href` is the path they lead to, `hash` the fragment.
 */
export function links(from: string | HTMLElement, url = typeof from === 'string' ? from : '/'): Link[] {
	const html = typeof from === 'string' ? page(from) : from;
	const base = `https://site.invalid${url}`;
	return html
		.querySelectorAll('a[href]')
		.map((el) => ({ el, to: new URL(el.getAttribute('href')!, base) }))
		.filter(({ to }) => to.origin === 'https://site.invalid')
		.map(({ el, to }) => ({ href: to.pathname, hash: to.hash.slice(1), text: text(el), el }));
}

/** The built Pages a visitor reaches from the home Page in at most `clicks` links. */
export function reachable(clicks: number): Set<string> {
	const seen = new Set(['/']);
	let edge = ['/'];
	for (let i = 0; i < clicks; i++) {
		const next = edge.filter((url) => url.endsWith('/') || url.endsWith('.html')).flatMap((url) => links(url).map((a) => a.href));
		edge = [...new Set(next)].filter((url) => !seen.has(url) && output(url) !== undefined);
		for (const url of edge) seen.add(url);
	}
	return seen;
}

/** An element's text, with runs of whitespace as one space. */
export function text(el: HTMLElement): string {
	return el.textContent.replace(/\s+/g, ' ').trim();
}

/** The main content of a Page: its `<main>`, or else its `<body>`. */
export function main(html: HTMLElement): HTMLElement {
	return html.querySelector('main') ?? html.querySelector('body') ?? html;
}

function git(...args: string[]): string {
	return execFileSync('git', args, { encoding: 'utf8' });
}

/** The project's files, by path, without what Git ignores. */
export function sources(): string[] {
	return git('ls-files', '--cached', '--others', '--exclude-standard', '-z')
		.split('\0')
		.filter((path) => path && existsSync(path))
		.sort();
}

export function source(path: string): string {
	return readFileSync(path, 'utf8');
}

/** Files the agent added or changed since the start. */
export function changed(): string[] {
	return git('status', '--porcelain', '--untracked-files=all', '--no-renames', '-z')
		.split('\0')
		.filter((record) => record && !record.slice(0, 2).includes('D'))
		.map((record) => record.slice(3));
}

// The checks every Eval task makes (#22). The others: nothing outside the
// project and nothing under `.yuanme/` reaches the grader, because it runs
// in a fresh sandbox that gets only the agent's changes to files Git
// tracks (experiment.ts).
test('the Site builds with --strict-budgets', () => {
	const { code, diagnostics } = build();
	expect(code, diagnostics.map((d) => d.rendered).join('\n\n')).toBe(0);
});

test('no npm package was added', () => {
	// The harness needs the root package.json, so it may stay as it was.
	const npm = changed().filter((path) =>
		['package.json', 'bun.lock', 'bun.lockb', 'pnpm-lock.yaml', 'yarn.lock'].includes(basename(path)),
	);
	expect(npm).toEqual([]);
});
