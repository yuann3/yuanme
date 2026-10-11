// Does NavigateEvent.scroll() (scroll:'manual') reset window scroll on push and restore on traverse?
// Synthetic page via page.route, so no Site router interferes.
import { firefox, chromium, webkit } from 'playwright';
const html = `<!doctype html><body style="min-height:4000px"><a id=a href="/two">two</a><script>
window.log=[];
navigation.addEventListener('navigate',e=>{ if(!e.canIntercept) return; e.intercept({scroll:'manual',async handler(){
 await new Promise(r=>setTimeout(r,30)); const b=scrollY; let t='ok'; try{e.scroll()}catch(x){t='threw '+x.name}
 await new Promise(r=>requestAnimationFrame(()=>requestAnimationFrame(r)));
 log.push(e.navigationType+' '+location.pathname+' before='+b+' scroll()='+t+' after='+scrollY);}})});
</script>`;
const T = <T,>(p: Promise<T>, ms = 20000) => Promise.race([p, new Promise<T>((_, j) => setTimeout(() => j(new Error('timeout')), ms))]);
for (const [n, bt] of [['firefox', firefox], ['chromium', chromium], ['webkit', webkit]] as const) {
	const b = await bt.launch();
	try {
		const p = await b.newPage({ viewport: { width: 400, height: 800 } });
		await p.route('http://t.test/**', (r) => r.fulfill({ contentType: 'text/html', body: html }));
		await p.goto('http://t.test/one');
		const r = await T(p.evaluate(async () => {
			const w = (ms: number) => new Promise((r) => setTimeout(r, ms));
			scrollTo(0, 600); await w(100);
			await (navigation as any).navigate('/two').finished; await w(150);
			scrollTo(0, 250); await w(100);
			await (navigation as any).back().finished; await w(150);
			await (navigation as any).forward().finished; await w(150);
			return (window as any).log;
		}));
		console.log(n, b.version(), JSON.stringify(r));
	} catch (e) { console.log(n, 'ERR', String(e).slice(0, 200)); }
	await b.close();
}
for (const [label, prefs, headless] of [
	['default', {}, true], ['force', { 'webgl.force-enabled': true, 'webgl.disabled': false }, true],
	['headed(xvfb) force', { 'webgl.force-enabled': true }, false],
] as const) {
	try {
		const b = await firefox.launch({ firefoxUserPrefs: prefs as any, headless });
		const p = await b.newPage();
		const gl = await T(p.evaluate(() => { const c = document.createElement('canvas').getContext('webgl'); return c ? (c.getParameter(c.RENDERER) as string) : null; }));
		console.log('ff webgl', label, gl); await b.close();
	} catch (e) { console.log('ff webgl', label, 'ERR', String(e).slice(0, 120)); }
}
process.exit(0);
