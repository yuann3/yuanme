// Runs experiments of the Agent eval with real agents and models (#22),
// and writes each one's summary to results/ENGINE-VERSION/EXPERIMENT.json.
// It costs money: each run is a model API session.
//
//   node lib/run.ts --yuanme LINUX-YUANME [--experiment NAME]... [--treatment T]... [--dry]
//
// EVAL_FILTER picks tasks by prefix, such as `create/`, and EVAL_RUNS the
// runs per task, 3 by default. --dry stages the tasks, checks each config
// and prints what would run, and calls no model.
import { execFileSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { parseArgs } from 'node:util';
import {
	type EvalRunData,
	type EvalSummary,
	getAgent,
	loadAllFixtures,
	parseTranscript,
	resolveConfig,
	runExperiment,
	validateConfig,
} from '@vercel/agent-eval';
import { config, EXPERIMENTS } from './experiment.ts';
import { stage } from './stage.ts';

const { values: args } = parseArgs({
	options: {
		yuanme: { type: 'string' },
		experiment: { type: 'string', multiple: true, default: [] },
		treatment: { type: 'string', multiple: true, default: [] },
		// Tasks run at once, each with all its runs: 2 tasks × 3 runs is 6
		// sandboxes, which a 4-core runner holds.
		parallel: { type: 'string', default: '2' },
		dry: { type: 'boolean', default: false },
	},
});
if (!args.yuanme) throw new Error('--yuanme names the Linux yuanme the sandboxes run');
const yuanme = resolve(args.yuanme);
process.env.YUANME_LINUX_BIN = yuanme;
const filter = process.env.EVAL_FILTER ?? '';

const chosen = EXPERIMENTS.filter(
	(e) =>
		(args.experiment.length === 0 || args.experiment.includes(e.name)) &&
		(args.treatment.length === 0 || args.treatment.includes(e.treatment)),
);
if (chosen.length === 0) throw new Error('no experiment matches');

// The fixtures are made by a yuanme that runs here: the Linux one on Linux,
// else the one on PATH.
const host = process.platform === 'linux' ? yuanme : execFileSync('which', ['yuanme'], { encoding: 'utf8' }).trim();
const engine = execFileSync(host, ['--version'], { encoding: 'utf8' }).trim().replace(/^yuanme /, '');
const dir = mkdtempSync(join(tmpdir(), 'yuanme-evals-'));
await stage(host, join(dir, 'evals'), filter);
const { fixtures, errors } = loadAllFixtures(join(dir, 'evals'));
if (errors.length) throw new Error(errors.map((e) => e.message).join('\n'));
console.log(`${fixtures.length} Eval tasks, staged with yuanme ${engine} in ${dir}`);

/** The run never reached the grader: the sandbox, the agent CLI or the API failed. */
const infra = (run: EvalRunData) =>
	run.result.status === 'failed' && !run.outputContent?.eval && !/timed? ?out|timeout/i.test(run.result.error ?? '');

const CODE = /(?:error|warning)\[([a-z]+\/[a-z0-9-]+)\]|\\*"code\\*"\s*:\s*\\*"([a-z]+\/[a-z0-9-]+)/g;

/** What #22 records of one run, beyond agent-eval's own result. */
function record(run: EvalRunData, agent: string) {
	const raw = run.transcript ?? '';
	const commands = raw ? parseTranscript(raw, agent).summary.shellCommands : [];
	const yuanmeCommands = commands.map((c) => c.command).filter((c) => /(^|[\s;&|(])yuanme\s/.test(c));
	const firstBuild = commands.findIndex(
		(c) => /(^|[\s;&|(])yuanme\s+(build|check)\b/.test(c.command) && (c.success ?? c.exitCode === 0),
	);
	const final = /^YUANME_EVAL_FINAL (.*)$/m.exec((run.outputContent?.eval ?? '').replace(/\x1b\[[0-9;]*m/g, ''));
	const last = final ? JSON.parse(final[1]) : undefined;
	return {
		status: run.result.status,
		error: run.result.error,
		duration: run.result.duration,
		usage: run.result.usage,
		cli: run.result.provenance?.agentCliVersion,
		yuanmeCommands,
		usedJson: yuanmeCommands.some((c) => /--json\b/.test(c)),
		usedDocs: yuanmeCommands.some((c) => /\byuanme\s+docs\b/.test(c)) || /\.yuanme\/docs\//.test(raw),
		hit: [...new Set([...raw.matchAll(CODE)].map((m) => m[1] ?? m[2]))].sort(),
		// The codes the grader's build still reported, or null if it never built.
		finalCodes: last ? [...new Set<string>(last.codes)].sort() : null,
		// 1-based: the shell commands the agent ran up to its first build or
		// check that passed.
		commandsToFirstBuild: firstBuild < 0 ? null : firstBuild + 1,
		budgets: last?.budgets ?? null,
	};
}

/** Runs `fixtures` of an experiment, retrying a run that never reached the grader up to 3 times. */
async function runTasks(name: string, exp: ReturnType<typeof resolveConfig>, apiKey: string) {
	const out = new Map<string, EvalRunData[]>();
	const queue = [...fixtures];
	await Promise.all(
		Array.from({ length: Number(args.parallel) }, async () => {
			for (let fixture = queue.shift(); fixture; fixture = queue.shift()) {
				const runs: EvalRunData[] = [];
				let missing = exp.runs;
				for (let attempt = 0; missing > 0 && attempt <= 3; attempt++) {
					const results = await runExperiment({
						config: { ...exp, runs: missing } as never,
						fixtures: [fixture],
						apiKey,
						resultsDir: join(dir, 'results'),
						experimentName: name,
					});
					const done = results.evals[0].runs;
					for (const run of done.filter(infra)) {
						// #22: retried, never counted as a fail, and never silent.
						console.log(`::warning::${name} ${fixture.name}: a run never reached the grader (${run.result.error}); ${attempt < 3 ? 'retrying it' : 'giving up after 3 retries'}`);
					}
					runs.push(...done.filter((r) => !infra(r)));
					missing = exp.runs - runs.length;
				}
				out.set(fixture.name, runs);
				const passed = runs.filter((r) => r.result.status === 'passed').length;
				console.log(`${name} ${fixture.name}: ${passed}/${runs.length} passed`);
			}
		}),
	);
	return out;
}

function summarize(experiment: (typeof EXPERIMENTS)[number], tasks: Map<string, EvalRunData[]>) {
	const records = Object.fromEntries(
		[...tasks].sort().map(([task, runs]) => [task, runs.map((r) => record(r, experiment.cli))]),
	);
	const rate = (runs: { status: string }[]) => ({
		runs: runs.length,
		passed: runs.filter((r) => r.status === 'passed').length,
	});
	const groups: Record<string, { runs: number; passed: number; tasks: number; pass3: number }> = {};
	for (const [task, runs] of Object.entries(records)) {
		const group = task.split('/')[0];
		const g = (groups[group] ??= { runs: 0, passed: 0, tasks: 0, pass3: 0 });
		const r = rate(runs);
		g.runs += r.runs;
		g.passed += r.passed;
		g.tasks += 1;
		if (r.runs > 0 && r.passed === r.runs) g.pass3 += 1;
	}
	// The fix rate per code: of the runs whose agent hit a code, how many
	// ended with a build that no longer reports it.
	const codes: Record<string, { hit: number; fixed: number }> = {};
	for (const runs of Object.values(records)) {
		for (const run of runs) {
			for (const code of run.hit) {
				const c = (codes[code] ??= { hit: 0, fixed: 0 });
				c.hit += 1;
				if (run.finalCodes && !run.finalCodes.includes(code)) c.fixed += 1;
			}
		}
	}
	const all = Object.values(records).flat();
	const cost = all.reduce((sum, r) => sum + (r.usage?.costUsd ?? 0), 0);
	return {
		engine,
		experiment: experiment.name,
		agent: experiment.cli,
		cli: all.find((r) => r.cli)?.cli ?? null,
		model: experiment.model,
		effort: experiment.effort,
		treatment: experiment.treatment,
		filter,
		...rate(all),
		groups,
		codes,
		tokens: all.reduce((sum, r) => sum + (r.usage?.totalTokens ?? 0), 0),
		// Only what the CLIs report; Codex and Gemini CLI report no cost.
		costUsd: all.every((r) => r.usage?.costUsd !== undefined) ? cost : null,
		tasks: records,
	};
}

const results = resolve(import.meta.dirname, '../results', engine);
for (const experiment of chosen) {
	const exp = resolveConfig(validateConfig(config(experiment.name)));
	const runs = fixtures.length * exp.runs;
	const apiKey = process.env[getAgent(exp.agent).getApiKeyEnvVar()];
	if (args.dry) {
		console.log(`${experiment.name}: ${exp.agent} ${exp.model}, ${runs} runs${apiKey ? '' : `, no ${experiment.key}`}`);
		continue;
	}
	if (!apiKey) {
		console.log(`::warning::${experiment.key} is not set, so ${experiment.name} was skipped`);
		continue;
	}
	const summary = summarize(experiment, await runTasks(experiment.name, exp, apiKey));
	mkdirSync(results, { recursive: true });
	writeFileSync(join(results, `${experiment.name}.json`), `${JSON.stringify(summary, null, 2)}\n`);
	const create = summary.groups.create;
	console.log(`${experiment.name}: ${summary.passed}/${summary.runs} passed${create ? `, create ${create.passed}/${create.runs}` : ''}`);
}
