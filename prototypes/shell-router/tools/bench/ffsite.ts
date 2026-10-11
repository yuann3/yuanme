// Firefox on the real Site: why no Film context, and where does mobile window scroll go on push?
import { firefox, chromium } from 'playwright';
const H = process.env.HOST ?? 'localhost';
const init = () => {
	const L: string[] = ((window as any).__L = []);
	const gc = HTMLCanvasElement.prototype.getContext;
	HTMLCanvasElement.prototype.getContext = function (t: string, ...a: any[]) { const c = gc.call(this, t, ...a); L.push(`getContext(${t}) -> ${c ? 'ok' : 'null'}`); return c; } as any;
	const NE = (window as any).NavigateEvent;
	if (NE) { const s = NE.prototype.scroll; NE.prototype.scroll = function () { const b = scrollY; s.call(this); L.push(`scroll() ${this.navigationType} ${b} -> ${scrollY} (max ${document.documentElement.scrollHeight - innerHeight})`); }; }
	for (const n of ['astro:after-swap', 'astro:page-load']) document.addEventListener(n, () => L.push(`${n} scrollY=${scrollY} max=${document.documentElement.scrollHeight - innerHeight} state=${document.documentElement.dataset.state}`));
	addEventListener('scroll', () => L.push(`scroll evt ${scrollY}`), { passive: true });
};
for (const [n, bt] of [['firefox', firefox], ['chromium', chromium]] as const) {
	const b = await bt.launch();
	for (const mobile of [false, true]) {
		const ctx = await b.newContext(mobile ? { viewport: { width: 390, height: 844 }, hasTouch: true, ...(n === 'chromium' ? { isMobile: true } : {}) } : { viewport: { width: 1440, height: 900 } });
		await ctx.addInitScript(init);
		const p = await ctx.newPage();
		const cons: string[] = [];
		p.on('console', (m) => cons.push(m.type() + ': ' + m.text().slice(0, 150)));
		await p.goto(`http://${H}:4322/`);
		await p.waitForTimeout(1300);
		const media = await p.evaluate(() => ({ rm: matchMedia('(prefers-reduced-motion: reduce)').matches, mob: matchMedia('(max-width: 767px)').matches }));
		if (mobile) {
			await p.evaluate(() => scrollTo(0, 300)); await p.waitForTimeout(300);
			await p.evaluate(() => ((window as any).__L.push('--- click Writing at ' + scrollY), (document.querySelector('[data-nav-link][data-section=writing]') as HTMLElement).click()));
			await p.waitForTimeout(1500);
		}
		const L = await p.evaluate(() => (window as any).__L);
		console.log(`== ${n} ${mobile ? 'mobile' : 'desktop'} media=${JSON.stringify(media)}\n  ` + L.join('\n  ') + (cons.length ? '\n  console: ' + cons.join(' | ') : ''));
		await ctx.close();
	}
	await b.close();
}
process.exit(0);
