// Injected into every document by feel.ts, the same on both sides. Taken
// from the prototype's probe (#26), without its timings: it records the
// Film's WebGL contexts and canvas, and the Router's swap events.
(() => {
	const ss = sessionStorage;
	const P = (window.__probe = { events: [], gl: { calls: 0, lost: 0 } });

	const gc = HTMLCanvasElement.prototype.getContext;
	HTMLCanvasElement.prototype.getContext = function (type, ...a) {
		const ctx = gc.call(this, type, ...a);
		if (/webgl/.test(type) && ctx && !this.__probed) {
			this.__probed = true;
			this.__probeId = this.__probeId || String(performance.timeOrigin) + Math.random();
			P.gl.calls++;
			P.gl.ctx = ctx;
			// Per tab, so a full load's new context counts too.
			ss.setItem('__ctx', String(+(ss.getItem('__ctx') || 0) + 1));
			this.addEventListener('webglcontextlost', () => P.gl.lost++);
		}
		return ctx;
	};

	// `router-events`: Astro's events are astro:*, the Engine's yuanme:*.
	for (const prefix of ['astro:', 'yuanme:']) {
		for (const name of ['before-swap', 'after-swap', 'page-load']) {
			document.addEventListener(prefix + name, () => P.events.push([name, performance.now()]));
		}
	}
})();
