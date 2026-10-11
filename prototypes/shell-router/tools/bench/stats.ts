export type Row = Record<string, any>;
export const pct = (xs: number[], p: number) => {
	const s = xs.filter((x) => Number.isFinite(x)).sort((a, b) => a - b);
	if (!s.length) return NaN;
	return s[Math.min(s.length - 1, Math.ceil(p * s.length) - 1)];
};
const f = (x: number) => (Number.isFinite(x) ? Math.round(x).toString() : '-');
const f3 = (x: number) => (Number.isFinite(x) ? x.toFixed(3) : '-');
export function summary(rows: Row[]) {
	const groups = new Map<string, Row[]>();
	for (const r of rows) {
		const k = `${r.step}|${r.variant}`;
		groups.set(k, [...(groups.get(k) || []), r]);
	}
	const lines = ['step | variant | n | kinds | painted med/p95 | settled med/p95 | film first frame med/p95 | GL init med/p95 | CLS max (all / counted) | ctx lost'];
	for (const [k, rs] of [...groups].sort()) {
		const [step, variant] = k.split('|');
		const g = (key: string) => rs.map((r) => r[key]);
		const film = rs.map((r) => r.film?.firstFrame ?? NaN);
		const init = rs.map((r) => r.film?.init ?? NaN);
		const kinds = [...new Set(g('kind'))].join('+');
		lines.push([step, variant, rs.length, kinds,
			`${f(pct(g('painted'), .5))} / ${f(pct(g('painted'), .95))}`,
			`${f(pct(g('settled'), .5))} / ${f(pct(g('settled'), .95))}`,
			`${f(pct(film, .5))} / ${f(pct(film, .95))}`,
			`${f(pct(init, .5))} / ${f(pct(init, .95))}`,
			`${f3(Math.max(...g('clsAll')))} / ${f3(Math.max(...g('cls')))}`,
			rs.some((r) => r.lost) ? 'LOST' : 'no',
		].join(' | '));
	}
	return lines.join('\n');
}
