// The grader self-test (#22): with no agent and no model, each Eval task's
// grader fails its start and each of its known-bad end states, and passes
// its solution. Each is graded as the sandbox grades an agent's work: the
// fixture's files as agent-eval uploads them, its first commit, the
// default treatment, then the end state laid over it.
//
//   node lib/selftest.ts --yuanme ../target/debug/yuanme [--filter create/]
import { execFile, execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { availableParallelism, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { parseArgs, promisify } from 'node:util';
import { collectLocalFiles, splitTestFiles } from '@vercel/agent-eval';
import { overlay, TASKS, tasks, treat } from './project.ts';
import { fixture, withYuanme } from './stage.ts';

const { values: args } = parseArgs({
	options: {
		yuanme: { type: 'string', default: join(import.meta.dirname, '../../target/debug/yuanme') },
		filter: { type: 'string', default: '' },
	},
});
const env = withYuanme(args.yuanme);
const root = mkdtempSync(join(tmpdir(), 'yuanme-selftest-'));
const nodeModules = join(import.meta.dirname, '../node_modules');

type Case = { task: string; name: string; pass: boolean; layers: [string, string][] };

const cases: Case[] = [];
for (const task of tasks().filter((t) => t.startsWith(args.filter))) {
	const dir = join(TASKS, task);
	const solution: [string, string] = [join(dir, 'solution'), join(dir, 'solution.sh')];
	cases.push({ task, name: 'start', pass: false, layers: [] });
	cases.push({ task, name: 'solution', pass: true, layers: [solution] });
	const bad = join(dir, 'bad');
	for (const name of existsSync(bad) ? readdirSync(bad) : []) {
		cases.push({ task, name: `bad/${name}`, pass: false, layers: [solution, [join(bad, name), '']] });
	}
}

const fixtures = new Map<string, string>();
for (const task of new Set(cases.map((c) => c.task))) {
	const dir = join(root, 'fixtures', task);
	await fixture(task, dir, env);
	fixtures.set(task, dir);
}

/** The project as the grader sees it after an agent left it in `c`'s state. */
async function project(c: Case): Promise<string> {
	const dir = join(root, 'cases', c.task, c.name);
	mkdirSync(dir, { recursive: true });
	const { workspaceFiles, testFiles } = splitTestFiles(await collectLocalFiles(fixtures.get(c.task)!));
	const write = (files: { path: string; content: Buffer | string }[]) => {
		for (const file of files) {
			mkdirSync(dirname(join(dir, file.path)), { recursive: true });
			writeFileSync(join(dir, file.path), file.content);
		}
	};
	write(workspaceFiles);
	// What agent-eval's initGitAndCommit does.
	writeFileSync(join(dir, '.gitignore'), 'node_modules/\n__agent_eval__/\n');
	const git = (...a: string[]) => execFileSync('git', a, { cwd: dir, env, stdio: 'ignore' });
	git('init', '-q');
	git('-c', 'user.email=agent-eval@localhost', '-c', 'user.name=agent-eval', 'add', '.');
	git('-c', 'user.email=agent-eval@localhost', '-c', 'user.name=agent-eval', 'commit', '-q', '-m', 'init');
	execFileSync('bash', ['-euo', 'pipefail', '-c', treat('default')], {
		cwd: dir,
		env: { ...env, GIT_AUTHOR_NAME: 'agent-eval', GIT_AUTHOR_EMAIL: 'agent-eval@localhost', GIT_COMMITTER_NAME: 'agent-eval', GIT_COMMITTER_EMAIL: 'agent-eval@localhost' },
		stdio: 'ignore',
	});
	for (const [from, script] of c.layers) overlay(dir, from, script, env);
	// What validation adds: the grader, vitest, and its config.
	write(testFiles.filter((f) => f.path === 'EVAL.ts'));
	symlinkSync(nodeModules, join(dir, 'node_modules'));
	writeFileSync(
		join(dir, 'vitest.config.ts'),
		"import { defineConfig } from 'vitest/config';\nexport default defineConfig({ test: { include: ['EVAL.ts'], testTimeout: 900000 } });\n",
	);
	return dir;
}

async function grade(c: Case): Promise<string | undefined> {
	const dir = await project(c);
	const vitest = join(nodeModules, 'vitest/vitest.mjs');
	const passed = await promisify(execFile)(process.execPath, [vitest, 'run', 'EVAL.ts'], { cwd: dir, env, maxBuffer: 64 << 20 }).then(
		() => true,
		(e: { stdout: string; stderr: string }) => {
			output.set(c, `${e.stdout}\n${e.stderr}`);
			return false;
		},
	);
	if (passed === c.pass) return undefined;
	return passed ? 'the grader passed it' : `the grader failed it:\n${output.get(c)}`;
}

const output = new Map<Case, string>();
const failures: string[] = [];
const queue = [...cases];
await Promise.all(
	Array.from({ length: availableParallelism() }, async () => {
		for (let c = queue.shift(); c; c = queue.shift()) {
			const problem = await grade(c);
			const label = `${c.task} ${c.name} ${c.pass ? 'passes' : 'fails'}`;
			// Which of the grader's tests failed it, to see it fails for the right reason.
			const failed = [...(output.get(c) ?? '').matchAll(/^ {3}× (.+?)(?: \d+ms)?$/gm)].map((m) => m[1]);
			console.log(`${problem ? 'FAIL' : 'ok  '} ${label}${failed.length ? `: ${failed.join('; ')}` : ''}`);
			if (problem) failures.push(`${label}: ${problem}`);
		}
	}),
);

if (failures.length > 0) {
	console.error(`\n${failures.join('\n\n')}`);
	console.error(`\n${failures.length} of ${cases.length} cases wrong; projects kept in ${root}`);
	process.exit(1);
}
rmSync(root, { recursive: true, force: true });
console.log(`\n${cases.length} cases right, ${new Set(cases.map((c) => c.task)).size} Eval tasks`);
