// Side-by-side server for prototype #26. Usage: bun serve.ts [dir-with-dists]
// Serves each build like Netlify does (/about -> 301 /about/, 404.html).
//   :4321  Astro <ClientRouter /> (stock eyuan.me)
//   :4322  prototype Navigation API router
//   :4323  prototype with the Navigation API hidden -> plain full page loads
//   :4324  prototype + History API driver, Navigation API hidden
import { existsSync, statSync } from 'node:fs';
import { join } from 'node:path';

const root = process.argv[2] ?? join(import.meta.dir, 'out');
const HIDE_NAV = '<script>Object.defineProperty(window,"navigation",{value:undefined,configurable:true})</script>';
const sites = [
	{ port: 4321, dir: 'astro-dist', label: 'Astro ClientRouter' },
	{ port: 4322, dir: 'proto-dist', label: 'prototype router (Navigation API)' },
	{ port: 4323, dir: 'proto-dist', label: 'prototype, Navigation API hidden: full page loads', hide: true },
	{ port: 4324, dir: 'proto-hist-dist', label: 'prototype + History API driver, Navigation API hidden', hide: true },
];

for (const s of sites) {
	const base = join(root, s.dir);
	if (!existsSync(base)) {
		console.log(`skip :${s.port} (${base} missing)`);
		continue;
	}
	Bun.serve({
		port: s.port,
		async fetch(req) {
			const url = new URL(req.url);
			let path = decodeURIComponent(url.pathname);
			let file = join(base, path);
			if (!file.startsWith(base)) return new Response('no', { status: 403 });
			if (existsSync(file) && statSync(file).isDirectory()) {
				if (!path.endsWith('/')) return Response.redirect(url.origin + path + '/' + url.search, 301);
				file = join(file, 'index.html');
			}
			let status = 200;
			if (!existsSync(file)) {
				file = join(base, '404.html');
				status = 404;
			}
			const html = file.endsWith('.html');
			const headers = { 'cache-control': html ? 'public,max-age=0,must-revalidate' : 'public, max-age=14400, must-revalidate' };
			if (html && s.hide) {
				const text = (await Bun.file(file).text()).replace('<head>', '<head>' + HIDE_NAV);
				return new Response(text, { status, headers: { ...headers, 'content-type': 'text/html; charset=utf-8' } });
			}
			return new Response(Bun.file(file), { status, headers });
		},
	});
	console.log(`http://localhost:${s.port}/  ${s.label}`);
}
