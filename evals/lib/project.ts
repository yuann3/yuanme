// How an Eval task's project is made (#22), shared by the sandbox
// (experiment.ts) and the grader self-test (selftest.ts), so the self-test
// grades the same projects the agents get.
//
// A task is `tasks/GROUP/NAME/`:
//   PROMPT.md     one sentence, as an Author would ask
//   EVAL.ts       the grader, never shown to the agent
//   start/        laid over what `yuanme init` writes (the blog Starter)
//   start.sh      optional: run after that, for what an overlay can't do
//   solution/     a known-good end state, laid over the start
//   solution.sh   optional: run after that, such as to delete a file
//   bad/NAME/     optional: known-bad end states, each laid over the solution
import { execFileSync } from 'node:child_process';
import { cpSync, existsSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

export const TASKS = join(import.meta.dirname, '../tasks');

export const TREATMENTS = ['default', 'bare', 'no-skills'] as const;
export type Treatment = (typeof TREATMENTS)[number];

/** Each task, as `GROUP/NAME`. */
export function tasks(): string[] {
	return readdirSync(TASKS)
		.flatMap((group) => readdirSync(join(TASKS, group)).map((name) => `${group}/${name}`))
		.sort();
}

function sh(dir: string, script: string, env: NodeJS.ProcessEnv) {
	execFileSync('bash', ['-euo', 'pipefail', script], { cwd: dir, env, stdio: ['ignore', 'pipe', 'inherit'] });
}

/** Lays `from`, if it exists, over `dir`, then runs `script`, if it exists. */
export function overlay(dir: string, from: string, script: string, env: NodeJS.ProcessEnv) {
	if (existsSync(from)) cpSync(from, dir, { recursive: true });
	if (existsSync(script)) sh(dir, script, env);
}

/**
 * Writes the project an agent starts `task` from into `dir`, as the
 * `yuanme` on env's PATH makes it, with the files a fixture of
 * `@vercel/agent-eval` needs. Every treatment starts from this; the
 * treatment script removes what it leaves out.
 */
export function start(task: string, dir: string, env: NodeJS.ProcessEnv) {
	// #70: every task starts from what `yuanme init` writes, so the tasks
	// run against whatever the Starter is.
	execFileSync('yuanme', ['init', '--quiet', dir], { env, stdio: ['ignore', 'ignore', 'inherit'] });
	overlay(dir, join(TASKS, task, 'start'), join(TASKS, task, 'start.sh'), env);
	// Made again by the first command the agent runs.
	for (const path of ['.yuanme/cache', '.yuanme/report.json', '.yuanme/report.md', 'dist']) {
		rmSync(join(dir, path), { recursive: true, force: true });
	}
	// agent-eval needs it, and runs `npm install` in it before the agent
	// starts. The grader checks it stays as it is.
	writeFileSync(join(dir, 'package.json'), '{ "private": true, "type": "module" }\n');
}

/**
 * The shell script that finishes the start project in its sandbox, run
 * from the project root after agent-eval's first commit (`git init`, all
 * files, and a `.gitignore` of its own in place of the project's).
 */
export function treat(treatment: Treatment): string {
	const remove = {
		default: '',
		// The binary alone: an agent learns from `yuanme --help` and `yuanme docs`.
		bare: 'rm -rf AGENTS.md CLAUDE.md .agents .claude .yuanme',
		'no-skills': 'rm -rf .agents/skills .claude/skills && rmdir .agents .claude',
	}[treatment];
	return `
# Fetches the Managed tools the start uses, such as tsc, into the user's
# cache, so the first check an agent runs doesn't wait for them. It also
# makes .yuanme/ again for this binary.
yuanme check --quiet > /dev/null 2>&1 || true
${remove}
# What yuanme init puts in .gitignore, which agent-eval replaced.
printf '.yuanme/\\ndist/\\n' >> .gitignore
git rm -r -q --cached --ignore-unmatch .yuanme
git add -A
git commit -q --amend --no-edit
`;
}
