// Redirect adoption: navigate to /about (server 301s to /about/) through the router.
// Usage: bun bench/redirect.ts <chromium|firefox|webkit>
import { chromium, firefox, webkit } from 'playwright';
const n = process.argv[2] ?? 'chromium';
const H = process.env.HOST ?? 'localhost';
const b = await (n === 'webkit' ? webkit : n === 'firefox' ? firefox : chromium).launch();
for (const port of [4321, 4322]) {
	const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
	await p.goto(`http://${H}:${port}/blog/`);
	await p.waitForTimeout(1300);
	const r = await p.evaluate(async () => {
		const ev: string[] = [];
		const nav = (window as any).navigation;
		nav.addEventListener('navigateerror', (e: any) => ev.push('navigateerror ' + e.message));
		nav.addEventListener('navigatesuccess', () => ev.push('navigatesuccess'));
		for (const t of ['astro:after-swap', 'astro:page-load']) document.addEventListener(t, () => ev.push(t + ' ' + location.pathname));
		const before = nav.entries().length;
		const timeOrigin = performance.timeOrigin;
		const a = Object.assign(document.createElement('a'), { href: '/about' });
		document.body.append(a); a.click();
		await new Promise((r) => setTimeout(r, 1500));
		return {
			path: location.pathname, sameDocument: performance.timeOrigin === timeOrigin,
			current: (document.querySelector('[data-nav-link][aria-current]') as HTMLElement)?.dataset.section,
			title: document.title.split(' | ')[0], contents: document.querySelectorAll('[data-panel-content]').length,
			entriesAdded: nav.entries().length - before, currentEntryUrl: new URL(nav.currentEntry.url).pathname, ev,
		};
	});
	const back = await p.evaluate(async () => { history.back(); await new Promise((r) => setTimeout(r, 1300)); return location.pathname; });
	console.log(n, b.version(), port === 4321 ? 'astro' : 'proto', JSON.stringify({ ...r, afterBack: back }));
	await p.close();
}
await b.close();
process.exit(0);
