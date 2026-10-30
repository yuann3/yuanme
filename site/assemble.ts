// Writes the docs site as it is deployed (#38) into OUT: a release's Site
// at /, and main's at /next/, each built with --strict and indexed by
// Pagefind. .github/workflows/docs.yml deploys it; the starters CI job
// checks it in a browser.
//
//   node site/assemble.ts --out site/dist \
//     --root DIR --root-yuanme BIN --version vX.Y.Z \
//     --next DIR --next-yuanme BIN
//
// DIR is a checkout's site/, and BIN the yuanme built from that checkout.
// Before the first release, both are main's, and --version is "main".
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { homedir, tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';

const { values: args } = parseArgs({
	options: {
		out: { type: 'string' },
		root: { type: 'string' },
		'root-yuanme': { type: 'string' },
		version: { type: 'string' },
		next: { type: 'string' },
		'next-yuanme': { type: 'string' },
	},
});
for (const name of ['out', 'root', 'root-yuanme', 'version', 'next', 'next-yuanme'] as const) {
	if (!args[name]) throw new Error(`--${name} is required`);
}

/** Pagefind, a standalone binary (#38), at a pinned version, by its release archive's SHA-256. */
const PAGEFIND = 'v1.5.2';
const ARCHIVES: Record<string, [string, string]> = {
	'linux-x64': ['x86_64-unknown-linux-musl', 'afb824a9e7f64905a934900481cea5be679c03975e527329e0e5e6cc70f5feda'],
	'linux-arm64': ['aarch64-unknown-linux-musl', 'f50ec608bcbf431cebd84e0efa3a5b041ee63df2ad81f138023e0cfd2f509424'],
	'darwin-arm64': ['aarch64-apple-darwin', '7286f394a349bd37677d44a65a20078a02b1747da0b0814d83403bf86be17abe'],
	'darwin-x64': ['x86_64-apple-darwin', '26f51b4ba921897142338fb13b836420696e251fe197b1782ea7981de311156d'],
};

/** Pagefind's binary, downloaded and checked once into the user's cache. */
async function pagefind(): Promise<string> {
	const archive = ARCHIVES[`${process.platform}-${process.arch}`];
	if (!archive) throw new Error(`no Pagefind for ${process.platform}-${process.arch}`);
	const [target, sha256] = archive;
	const dir = join(process.env.XDG_CACHE_HOME ?? join(homedir(), '.cache'), 'yuanme-docs', `pagefind-${PAGEFIND}`);
	const bin = join(dir, 'pagefind');
	if (existsSync(bin)) return bin;
	const url = `https://github.com/Pagefind/pagefind/releases/download/${PAGEFIND}/pagefind-${PAGEFIND}-${target}.tar.gz`;
	const response = await fetch(url);
	if (!response.ok) throw new Error(`${url}: ${response.status}`);
	const bytes = Buffer.from(await response.arrayBuffer());
	const got = createHash('sha256').update(bytes).digest('hex');
	if (got !== sha256) throw new Error(`${url} has SHA-256 ${got}, not ${sha256}`);
	mkdirSync(dir, { recursive: true });
	writeFileSync(join(dir, 'pagefind.tar.gz'), bytes);
	execFileSync('tar', ['-xzf', 'pagefind.tar.gz', 'pagefind'], { cwd: dir });
	rmSync(join(dir, 'pagefind.tar.gz'));
	return bin;
}

/** `text` with `from` replaced by `to`, which must be in it once. */
function replaceOnce(text: string, from: string | RegExp, to: string, file: string): string {
	const count = text.split(from).length - 1;
	if (count !== 1) throw new Error(`${file} has ${count} of ${from}, where assemble.ts expects one`);
	return text.replace(from, to);
}

/**
 * The Site of the project `dir` as `yuanme` builds it, for `version` and
 * served from `base`, at the address `url`, with its Pagefind index in
 * `base`pagefind/. Every Route moves under `base`; the Templates link from
 * `site.extra.base`, and --strict fails a link that doesn't.
 */
function build(dir: string, yuanme: string, version: string, base: string, url: string, work: string): string {
	const project = join(work, base === '/' ? 'root' : 'next');
	cpSync(resolve(dir), project, {
		recursive: true,
		filter: (path) => !/[\\/](dist|\.yuanme|node_modules)$/.test(path),
	});
	const config = join(project, 'yuanme.toml');
	let toml = readFileSync(config, 'utf8');
	toml = replaceOnce(toml, /^url = ".*"$/m, `url = "${url}"`, config);
	toml = replaceOnce(toml, /^version = "main"$/m, `version = "${version}"`, config);
	toml = replaceOnce(toml, /^base = "\/"$/m, `base = "${base}"`, config);
	toml = toml.replace(/^((?:list_)?route|path) = "\//gm, `$1 = "${base}`);
	writeFileSync(config, toml);
	execFileSync(resolve(yuanme), ['build', '--strict', '-q'], { cwd: project, stdio: 'inherit' });
	return join(project, 'dist');
}

/** Copies each file under `from` into `to`; one already there must have the same bytes. */
function merge(from: string, to: string) {
	for (const name of readdirSync(from)) {
		const [src, dst] = [join(from, name), join(to, name)];
		if (statSync(src).isDirectory()) {
			mkdirSync(dst, { recursive: true });
			merge(src, dst);
		} else if (!existsSync(dst)) {
			cpSync(src, dst);
		} else if (!readFileSync(src).equals(readFileSync(dst))) {
			throw new Error(`${dst} differs between the two builds`);
		}
	}
}

const work = mkdtempSync(join(tmpdir(), 'yuanme-docs-'));
// The address is main's, so a new domain needn't wait for a release.
const url = /^url = "(.*)"$/m.exec(readFileSync(join(args.next!, 'yuanme.toml'), 'utf8'))?.[1];
if (!url) throw new Error(`${args.next}/yuanme.toml has no url`);
const root = build(args.root!, args['root-yuanme']!, args.version!, '/', url, work);
const next = build(args.next!, args['next-yuanme']!, 'main', '/next/', url, work);
const bin = await pagefind();
for (const [site, subdir] of [
	[root, 'pagefind'],
	[next, 'next/pagefind'],
]) {
	execFileSync(bin, ['--site', site, '--output-subdir', subdir, '--silent'], { stdio: 'inherit' });
}

const out = resolve(args.out!);
rmSync(out, { recursive: true, force: true });
cpSync(root, out, { recursive: true });
// main's Pages, and the hashed files they load. The rest, such as
// _headers and the sitemap, is the release's.
for (const dir of ['next', 'assets']) {
	mkdirSync(join(out, dir), { recursive: true });
	merge(join(next, dir), join(out, dir));
}
rmSync(work, { recursive: true, force: true });
console.log(`docs site: ${args.version} at /, main at /next/, in ${out}`);
