// Writes the project `@vercel/agent-eval` runs: each Eval task as a
// fixture, made by the yuanme under test, and a stub for each experiment.
import { execFileSync } from 'node:child_process';
import { copyFileSync, mkdirSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { delimiter, dirname, join, resolve } from 'node:path';
import { build } from 'esbuild';
import { start, TASKS, tasks } from './project.ts';

/** PATH with the yuanme under test first. */
export function withYuanme(yuanme: string): NodeJS.ProcessEnv {
	const bin = dirname(resolve(yuanme));
	const version = execFileSync(resolve(yuanme), ['--version'], { encoding: 'utf8' }).trim();
	if (!version.startsWith('yuanme ')) throw new Error(`${yuanme} is not yuanme: ${version}`);
	return { ...process.env, PATH: `${bin}${delimiter}${process.env.PATH}` };
}

/**
 * The grader as the sandbox runs it: one file, with this folder's helpers
 * and node-html-parser in it, since the sandbox installs only vitest.
 */
export async function bundle(task: string): Promise<string> {
	const result = await build({
		entryPoints: [join(TASKS, task, 'EVAL.ts')],
		bundle: true,
		platform: 'node',
		format: 'esm',
		target: 'node22',
		external: ['vitest'],
		write: false,
		logLevel: 'error',
	});
	return result.outputFiles[0].text;
}

/** Writes `task`'s fixture into `dir`: its start, PROMPT.md and EVAL.ts. */
export async function fixture(task: string, dir: string, env: NodeJS.ProcessEnv) {
	rmSync(dir, { recursive: true, force: true });
	mkdirSync(dir, { recursive: true });
	start(task, dir, env);
	copyFileSync(join(TASKS, task, 'PROMPT.md'), join(dir, 'PROMPT.md'));
	writeFileSync(join(dir, 'EVAL.ts'), await bundle(task));
}

export async function stage(yuanme: string, out: string) {
	const env = withYuanme(yuanme);
	for (const task of tasks()) await fixture(task, join(out, 'evals', task), env);
	// agent-eval finds the fixtures beside the experiments' folder.
	const experiments = join(import.meta.dirname, '../experiments');
	rmSync(join(out, 'experiments'), { recursive: true, force: true });
	mkdirSync(join(out, 'experiments'));
	for (const file of readdirSync(experiments)) {
		writeFileSync(join(out, 'experiments', file), `export { default } from ${JSON.stringify(join(experiments, file))};\n`);
	}
}
