// The experiments of the Agent eval (#22): each tier-1 agent CLI, with its
// model and effort, in each treatment, run on every Eval task in Docker.
//
// The sandbox is agent-eval's node:24-slim, as the node user. `setup` puts
// the yuanme under test on its PATH, from YUANME_LINUX_BIN, and applies
// the treatment. Each run is graded in a fresh sandbox that gets only the
// agent's changes to files Git tracks, so nothing it did outside the
// project, under .yuanme/ or to the grader reaches it (#22's invariants).
import { readFileSync } from 'node:fs';
import { type ExperimentConfig, getAgent, type Sandbox } from '@vercel/agent-eval';
import { type Treatment, TREATMENTS, treat } from './project.ts';

type Cli = 'claude-code' | 'codex' | 'gemini';

/**
 * Each agent CLI, at the version the suite runs. agent-eval installs the
 * latest of each, so its install step is pointed at this one: the version
 * is part of each experiment's name, and a new release of a CLI is a
 * change made here.
 */
const CLIS: Record<Cli, { package: string; version: string; key: string }> = {
	'claude-code': { package: '@anthropic-ai/claude-code', version: '2.1.314', key: 'ANTHROPIC_API_KEY' },
	codex: { package: '@openai/codex', version: '0.166.0', key: 'OPENAI_API_KEY' },
	gemini: { package: '@google/gemini-cli', version: '0.66.0', key: 'GEMINI_API_KEY' },
};

/**
 * Tier 1 (#22): each vendor's frontier model in its own CLI, and Claude
 * Code with the mid-tier Claude model, at the prices of 2026-10-30.
 */
export const TIER1: { cli: Cli; model: string }[] = [
	{ cli: 'claude-code', model: 'claude-fable-5-1' },
	{ cli: 'claude-code', model: 'claude-sonnet-5-5' },
	{ cli: 'codex', model: 'gpt-6-astra' },
	{ cli: 'gemini', model: 'gemini-4-argon' },
];

export type Experiment = { name: string; cli: Cli; model: string; effort: string; treatment: Treatment; key: string };

/**
 * Every experiment, named as #22 asks: the CLI and its version, the model,
 * the effort and the treatment. #22's effort is high; agent-eval passes no
 * effort to Gemini CLI, which thinks as its model does by default.
 */
export const EXPERIMENTS: Experiment[] = TIER1.flatMap(({ cli, model }) =>
	TREATMENTS.map((treatment) => {
		const effort = cli === 'gemini' ? 'native' : 'high';
		const name = `${cli}-${CLIS[cli].version}-${model}-${effort}-${treatment}`;
		return { name, cli, model, effort, treatment, key: CLIS[cli].key };
	}),
);

for (const [cli, { package: name, version }] of Object.entries(CLIS)) {
	// The definition is the object the agent's run reads, so this changes
	// what it installs. agent-eval has no option for it but Claude Code's.
	const definition = getAgent(cli).definition;
	const install = definition.install.bind(definition);
	definition.install = (options) =>
		install(options).map((step) =>
			step.kind === 'command' && step.args?.[0] === 'install' && step.args.includes('-g')
				? { ...step, args: ['install', '-g', `${name}@${version}`] }
				: step,
		);
}

async function run(sandbox: Sandbox, script: string) {
	const result = await sandbox.runCommand('bash', ['-euo', 'pipefail', '-c', script]);
	if (result.exitCode !== 0) throw new Error(`setup failed (${result.exitCode}):\n${result.stdout}\n${result.stderr}`);
}

export function config(name: string): ExperimentConfig {
	const found = EXPERIMENTS.find((e) => e.name === name);
	if (!found) throw new Error(`no experiment named ${name}`);
	const { cli, model, treatment } = found;
	return {
		agent: cli,
		model: cli === 'codex' ? `${model}?reasoningEffort=high` : model,
		agentOptions: cli === 'claude-code' ? { effort: 'high' } : undefined,
		// Only what yuanme gives an agent: no skills the CLI ships with, and no
		// web. agent-eval can turn neither off for Gemini CLI.
		disableBundledSkills: cli === 'gemini' ? undefined : true,
		webResearch: false,
		// 3 runs, no early exit: the pass rate estimates pass@1, and pass^3 is
		// all three. 15 minutes a run, with no cap on turns.
		runs: Number(process.env.EVAL_RUNS ?? 3),
		earlyExit: false,
		timeout: 900,
		sandbox: 'docker',
		verifier: 'separate',
		copyFiles: 'changed',
		setup: async (sandbox) => {
			const bin = process.env.YUANME_LINUX_BIN;
			if (!bin) throw new Error('YUANME_LINUX_BIN must name the Linux yuanme to put in the sandbox');
			// writeFiles takes text, so the binary goes as base64.
			await sandbox.writeFiles({ '.yuanme-bin.b64': readFileSync(bin).toString('base64') });
			await run(
				sandbox,
				'dir="$(npm config get prefix)/bin" && mkdir -p "$dir" && base64 -d .yuanme-bin.b64 > "$dir/yuanme" && rm .yuanme-bin.b64 && chmod +x "$dir/yuanme" && yuanme --version',
			);
			await run(sandbox, treat(treatment));
		},
	};
}
