// Injected with page.addInitScript into every document, identical for every build.
// Records, relative to the last user action (click/keydown, or one the harness sets):
//   painted  - the first frame after the new panel content is in the DOM
//              (soft nav: after astro:after-swap; full load: after shell boot())
//   settled  - all finite animations (morph, reveals, boot) finished
//   film     - WebGL getContext -> first drawArrays -> first frame (full loads only)
// plus layout shifts, swap event order, page-load listener registrations, context counts.
(() => {
	const ss = sessionStorage;
	const now = () => performance.now();
	const epoch = (t) => performance.timeOrigin + t;
	const P = (window.__probe = { events: [], shifts: [], listeners: [], gl: { calls: 0, lost: 0 }, done: null });
	const actEpoch = () => +(ss.getItem('__act') || 0) || performance.timeOrigin;
	P.act = actEpoch();
	ss.removeItem('__act');
	P.ctxTab = +(ss.getItem('__ctx') || 0);

	const afterPaint = (cb) => requestAnimationFrame(() => {
		const mc = new MessageChannel();
		mc.port1.onmessage = () => cb(now());
		mc.port2.postMessage(0);
	});
	// Settled: no running finite animation (morph, reveals, boot) for two frames, capped at 4 s.
	const busy = () => document.getAnimations().some((a) => {
		const end = a.effect && a.effect.getComputedTiming().endTime;
		// the idle palette cycle tweens --film-* custom properties every 8.5 s; that is ambient, not navigation
		return a.playState === 'running' && Number.isFinite(end) && !String(a.transitionProperty || '').startsWith('--film');
	});
	const settle = () => new Promise((resolve) => {
		const start = now();
		let quiet = 0;
		const tick = () => {
			quiet = busy() ? 0 : quiet + 1;
			if (quiet >= 2 || now() - start > 4000) afterPaint(resolve);
			else requestAnimationFrame(tick);
		};
		requestAnimationFrame(tick);
	});
	const rel = (t) => epoch(t) - P.act;

	const gc = HTMLCanvasElement.prototype.getContext;
	HTMLCanvasElement.prototype.getContext = function (type, ...a) {
		const ctx = gc.call(this, type, ...a);
		if (/webgl/.test(type) && ctx && !this.__probed) {
			this.__probed = true;
			this.__probeId = this.__probeId || Math.random().toString(36).slice(2);
			P.gl.calls++;
			P.gl.getContextAt = now();
			P.gl.canvas = this;
			P.gl.ctx = ctx;
			ss.setItem('__ctx', String(+(ss.getItem('__ctx') || 0) + 1));
			this.addEventListener('webglcontextlost', () => P.gl.lost++);
			try {
				const ext = ctx.getExtension('WEBGL_debug_renderer_info');
				P.gl.renderer = ext ? ctx.getParameter(ext.UNMASKED_RENDERER_WEBGL) : ctx.getParameter(ctx.RENDERER);
			} catch {}
		}
		return ctx;
	};
	const da = WebGLRenderingContext.prototype.drawArrays;
	WebGLRenderingContext.prototype.drawArrays = function (...a) {
		const r = da.apply(this, a);
		if (!P.gl.firstDrawAt) {
			P.gl.firstDrawAt = now();
			afterPaint((t) => (P.gl.firstFrameAt = t));
		}
		return r;
	};

	const ael = EventTarget.prototype.addEventListener;
	EventTarget.prototype.addEventListener = function (type, fn, ...rest) {
		if (this === document && /page-load$/.test(type) && fn) {
			const src = String(fn);
			P.listeners.push(src.includes('data-copy-code') ? 'copybtn' : /data-pile/.test(src) ? 'photopile' : src.length < 40 ? src : 'other');
		}
		return ael.call(this, type, fn, ...rest);
	};

	for (const name of ['before-swap', 'after-swap', 'page-load']) {
		ael.call(document, 'astro:' + name, () => P.events.push([name, now()]));
	}

	window.addEventListener('click', (e) => { if (e.isTrusted) ss.setItem('__act', String(epoch(e.timeStamp))); }, true);
	window.addEventListener('keydown', (e) => { if (e.isTrusted && e.key === 'Escape') ss.setItem('__act', String(epoch(e.timeStamp))); }, true);

	const finish = async (kind, paintedAt) => {
		const settledAt = await settle();
		const film = P.gl.getContextAt != null && kind !== 'soft' ? {
			init: (P.gl.firstFrameAt ?? NaN) - P.gl.getContextAt,
			firstFrame: P.gl.firstFrameAt != null ? rel(P.gl.firstFrameAt) : null,
		} : null;
		P.done = { kind, painted: rel(paintedAt), settled: rel(settledAt), film, path: location.pathname };
	};

	// Soft navigation: the router fired after-swap.
	ael.call(document, 'astro:after-swap', () => {
		P.act = actEpoch();
		ss.removeItem('__act');
		P.done = null;
		const swapAt = now();
		afterPaint((t) => { P.swapAt = swapAt; finish('soft', t); });
	});

	// Full load: shell.ts boot() removes body[data-boot].
	new MutationObserver((recs) => {
		for (const r of recs) {
			if (r.attributeName === 'data-boot' && !r.target.hasAttribute('data-boot') && r.target === document.body && !P.booted) {
				P.booted = true;
				afterPaint((t) => {
					// wait for the film's first frame too (if it starts)
					let n = 0;
					const wait = () => (P.gl.getContextAt != null && P.gl.firstFrameAt == null && n++ < 600 ? setTimeout(wait, 5) : finish("full", t));
					wait();
				});
			}
		}
	}).observe(document, { subtree: true, attributes: true, attributeFilter: ['data-boot'] });

	window.addEventListener('pageshow', (e) => {
		if (!e.persisted) return;
		P.act = actEpoch();
		ss.removeItem('__act');
		P.done = null;
		afterPaint((t) => finish('bfcache', t));
	});

	try {
		new PerformanceObserver((l) => {
			for (const e of l.getEntries()) P.shifts.push([e.startTime, e.value, e.hadRecentInput, (e.sources || []).map((s) => s.node ? (s.node.className || s.node.nodeName || "").toString().split(" ")[0] : "?").join(",")]);
		}).observe({ type: 'layout-shift', buffered: true });
	} catch {}
})();
