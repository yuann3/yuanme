// Where does the dev soft swap spend its time? Full-document parse vs panel-fragment parse vs DOM replace.
import { chromium, webkit } from 'playwright';
import { pct } from './stats';
for (const [n, bt] of [['webkit', webkit], ['chromium', chromium]] as const) {
	const b = await bt.launch();
	const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
	await p.goto('http://localhost:4322/blog/extending-c-stdlib/');
	await p.waitForTimeout(1500);
	const r = await p.evaluate(async () => {
		const html = await (await fetch(location.href)).text();
		const frag = new DOMParser().parseFromString(html, 'text/html').querySelector('[data-panel-content]')!.outerHTML;
		const T: Record<string, number[]> = { parseDoc: [], parseFrag: [], replace: [], paintAfterReplace: [] };
		for (let i = 0; i < 30; i++) {
			await new Promise((r) => setTimeout(r, 50));
			let t = performance.now();
			new DOMParser().parseFromString(html, 'text/html');
			T.parseDoc.push(performance.now() - t);
			t = performance.now();
			const tpl = document.createElement('template'); tpl.innerHTML = frag;
			const el = tpl.content.firstElementChild!;
			T.parseFrag.push(performance.now() - t);
			await new Promise((r) => requestAnimationFrame(r));
			t = performance.now();
			document.querySelector('[data-panel-content]')!.replaceWith(el);
			T.replace.push(performance.now() - t);
			await new Promise((r) => requestAnimationFrame(() => { const m = new MessageChannel(); m.port1.onmessage = r; m.port2.postMessage(0); }));
			T.paintAfterReplace.push(performance.now() - t);
		}
		return { htmlBytes: html.length, fragBytes: frag.length, T };
	});
	const f = (xs: number[]) => `${Math.round(pct(xs, .5) * 10) / 10} / ${Math.round(pct(xs, .95) * 10) / 10}`;
	console.log(n, b.version(), `html ${r.htmlBytes} B, fragment ${r.fragBytes} B`, Object.entries(r.T).map(([k, v]) => `${k} ${f(v)}`).join(' | '));
	await b.close();
}
