// The types of what yuanme gives a Site's Scripts. yuanme writes this file
// to .yuanme/types/yuanme.d.ts on every run, so edits to it don't last;
// include it from tsconfig.json. `yuanme docs scripts` explains each one.

declare module 'yuanme:router' {
	/**
	 * Moves to `url`: through the Router when `[router] mode = "swap"`, or
	 * else as a full page load. `history: "replace"` replaces the current
	 * history entry instead of adding one.
	 */
	export function navigate(url: string, options?: { history?: 'auto' | 'push' | 'replace' }): void;

	/** `yuanme:before-swap`, fired on `document` before the Router swaps a Page in. */
	export interface BeforeSwapEvent extends Event {
		readonly from: URL;
		readonly to: URL;
		readonly navigationType: 'push' | 'replace' | 'traverse';
		/** The parsed Page that is about to go in. */
		readonly newDocument: Document;
		/** Swaps `newDocument` in. Replace it to change how the Page goes in. */
		swap: () => void;
	}
}

declare module 'yuanme:mount' {
	/** What a Sidecar script's exported `mount` gets on every Page load that rendered its Template. */
	export interface MountContext<Data extends object = Record<string, unknown>> {
		/** The Page's URL. */
		readonly url: URL;
		/** Aborts before the Router swaps the Page out. */
		readonly signal: AbortSignal;
		/** The Sidecar script's own object, which lasts from Page to Page until the next full load. */
		readonly data: Data;
	}

	/** A Sidecar script's `mount`, which may return a cleanup that runs when `ctx.signal` aborts. */
	export type Mount<Data extends object = Record<string, unknown>> = (ctx: MountContext<Data>) => void | (() => void);
}

interface DocumentEventMap {
	'yuanme:before-swap': import('yuanme:router').BeforeSwapEvent;
	'yuanme:after-swap': Event;
	'yuanme:page-load': Event;
}

// A Data file arrives as its value. yuanme doesn't type it yet.
declare module '*.yaml' {
	const value: any;
	export default value;
}
declare module '*.yml' {
	const value: any;
	export default value;
}
declare module '*.toml' {
	const value: any;
	export default value;
}
