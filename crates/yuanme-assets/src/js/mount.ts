// The Script lifecycle (ADR 0013): each Page's Script registers the Sidecar
// scripts of the Templates its Page rendered, and `mount` calls their
// exported `mount(ctx)` for the Page now in the document.

type Ctx = { url: URL; signal: AbortSignal; data: object };
type Sidecar = { mount?: (ctx: Ctx) => unknown };

// Keyed by each Page Script's URL, which its `<script>` names, so a Page
// swapped in again finds the modules its Script registered the first time:
// a module's top level runs once per document.
const pages = new Map<string, Sidecar[]>();
const data = new WeakMap<Sidecar, object>();
let controller: AbortController | undefined;
let cleanups: unknown[] = [];

export const page = (url: string, sidecars: Sidecar[]) => pages.set(url, sidecars);

export function mount() {
	controller = new AbortController();
	const url = new URL(location.href);
	for (const s of document.scripts) {
		for (const m of pages.get(s.src) || []) {
			if (!m.mount) continue;
			data.has(m) || data.set(m, {});
			// One Sidecar's mistake mustn't stop the others, and the error
			// still reaches the console and `error` listeners.
			try {
				cleanups.push(m.mount({ url, signal: controller.signal, data: data.get(m)! }));
			} catch (e) {
				reportError(e);
			}
		}
	}
}

export function unmount() {
	controller?.abort();
	for (const c of cleanups) {
		try {
			typeof c == 'function' && c();
		} catch (e) {
			reportError(e);
		}
	}
	cleanups = [];
}
