// What the pixel and behaviour checks share: serving a Site as its host
// does, answering every other origin from the Parity baseline's remote/
// snapshot, and the Parity exception ids.
import { createServer } from 'node:http';
import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { extname, join, relative, sep } from 'node:path';
import type { AddressInfo } from 'node:net';
import type { Browser, BrowserContext, BrowserContextOptions } from 'playwright';

export const EXCEPTIONS = join(import.meta.dirname, '../../docs/parity-exceptions.md');

/** The ids of docs/parity-exceptions.md's tables, as `cargo run -p parity` reads them. */
export function exceptionIds(file = EXCEPTIONS): Set<string> {
	const ids = new Set<string>();
	for (const line of readFileSync(file, 'utf8').split('\n')) {
		const m = /^\|\s*`([a-z0-9-]+)`\s*\|/.exec(line);
		if (m) ids.add(m[1]);
	}
	return ids;
}

/** Fails unless every id a rule names is in the exceptions file. */
export function checkRules(ids: Iterable<string>, file = EXCEPTIONS) {
	const known = exceptionIds(file);
	const unknown = [...new Set(ids)].filter((id) => !known.has(id));
	if (unknown.length) throw new Error(`the harness tolerates differences under ids that docs/parity-exceptions.md lacks: ${unknown.join(', ')}`);
}

const TYPES: Record<string, string> = {
	'.html': 'text/html; charset=utf-8',
	'.css': 'text/css; charset=utf-8',
	'.js': 'text/javascript; charset=utf-8',
	'.json': 'application/json',
	'.xml': 'application/xml',
	'.txt': 'text/plain; charset=utf-8',
	'.svg': 'image/svg+xml',
	'.png': 'image/png',
	'.jpg': 'image/jpeg',
	'.jpeg': 'image/jpeg',
	'.webp': 'image/webp',
	'.avif': 'image/avif',
	'.woff': 'font/woff',
	'.woff2': 'font/woff2',
	'.pdf': 'application/pdf',
};

export type Server = { origin: string; close: () => Promise<void> };

/** Serves a Site as Netlify served the baseline: `/x` redirects to `/x/`, and a missing file gets 404.html with a 404. */
export async function serve(root: string): Promise<Server> {
	const server = createServer((req, res) => {
		const url = new URL(req.url ?? '/', 'http://localhost');
		const path = decodeURIComponent(url.pathname);
		let file = join(root, path);
		if (relative(root, file).startsWith('..')) return void res.writeHead(403).end();
		if (existsSync(file) && statSync(file).isDirectory()) {
			if (!path.endsWith('/')) return void res.writeHead(301, { location: path + '/' + url.search }).end();
			file = join(file, 'index.html');
		}
		let status = 200;
		if (!existsSync(file)) {
			file = join(root, '404.html');
			status = 404;
		}
		res.writeHead(status, { 'content-type': TYPES[extname(file).toLowerCase()] ?? 'application/octet-stream', 'cache-control': 'no-store' });
		res.end(readFileSync(file));
	});
	await new Promise<void>((resolve) => server.listen(0, '127.0.0.1', resolve));
	const { port } = server.address() as AddressInfo;
	return { origin: `http://127.0.0.1:${port}`, close: () => new Promise((resolve) => server.close(() => resolve())) };
}

/** Every Page of a Site, as the URL it is served at. */
export function pages(root: string): string[] {
	const out: string[] = [];
	const walk = (dir: string) => {
		for (const name of readdirSync(dir).sort()) {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) walk(path);
			else if (name.endsWith('.html')) {
				const rel = '/' + relative(root, path).split(sep).join('/');
				out.push(rel.endsWith('/index.html') ? rel.slice(0, -'index.html'.length) : rel);
			}
		}
	};
	walk(root);
	return out;
}

type Remote = Record<string, { file: string; type: string }>;

/**
 * A browser context in which the Site's origin is `server` and every other
 * origin is answered from the baseline's remote/ snapshot: its Google Fonts
 * stylesheets, their font files, and the images Pages embed (#23). Anything
 * else is refused, and recorded in `refused`, so nothing depends on the
 * network.
 */
export async function context(browser: Browser, server: Server, remoteDir: string, options: BrowserContextOptions = {}) {
	const remote: Remote = JSON.parse(readFileSync(join(remoteDir, 'index.json'), 'utf8'));
	const ctx: BrowserContext = await browser.newContext(options);
	const refused: string[] = [];
	await ctx.route('**/*', (route) => {
		const url = route.request().url();
		if (url.startsWith(server.origin + '/')) return route.continue();
		const hit = remote[url];
		if (hit) return route.fulfill({ status: 200, contentType: hit.type, headers: { 'access-control-allow-origin': '*' }, body: readFileSync(join(remoteDir, hit.file)) });
		refused.push(url);
		return route.abort('blockedbyclient');
	});
	// The same pseudo-random sequence on both sides: PhotoPile scatters its
	// photos with Math.random.
	await ctx.addInitScript(() => {
		let s = 0x2545f491;
		Math.random = () => {
			s = (s + 0x6d2b79f5) | 0;
			let t = Math.imul(s ^ (s >>> 15), 1 | s);
			t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
			return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
		};
	});
	return { ctx, refused };
}
