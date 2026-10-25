// Regenerates what code highlighting is pinned to and checked against (#53):
// - src/highlight/shiki/: the grammars and Catppuccin themes the Reference
//   site's Shiki 3.19 runs, in ferriki-asset-gen's upstream layout, and
//   NOTICE with each one's licence from tm-grammars and tm-themes, the
//   packages Shiki 3.19 was built from;
// - tests/shiki/tokens.json: Shiki's tokens for every code block in the
//   corpus, which tests/highlight.rs must match character for character.
//
// Run from the repository root, with the Reference site checked out and
// installed (shiki 3.19.0), and npm on the PATH:
//   node crates/yuanme-render/tests/shiki/generate.mjs ../eyuan.me
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

const site = path.resolve(process.argv[2]);
const shiki = path.join(site, 'node_modules/shiki/dist');
// The versions in shiki 3.19.0's own lockfile.
const TM_GRAMMARS = 'tm-grammars@1.26.0';
const TM_THEMES = 'tm-themes@1.10.13';
const THEMES = ['catppuccin-latte', 'catppuccin-mocha'];
const out = 'crates/yuanme-render/src/highlight/shiki';

// The Engine embeds only permissive grammars, plus MPL-2.0 ones unmodified
// (ADR 0014), so GPL grammars are left out.
const allowed = (spdx) => !spdx?.startsWith('GPL');

const { bundledLanguagesInfo } = await import(path.join(shiki, 'langs.mjs'));
const grammars = new Map();
for (const info of bundledLanguagesInfo) {
  for (const g of (await info.import()).default) grammars.set(g.name, g);
}

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'yuanme-shiki-'));
const unpack = (spec) => {
  const tgz = execFileSync('npm', ['pack', '--silent', spec], { cwd: tmp }).toString().trim();
  const dir = path.join(tmp, spec.split('@')[0]);
  fs.mkdirSync(dir);
  execFileSync('tar', ['xzf', path.join(tmp, tgz), '-C', dir]);
  return path.join(dir, 'package');
};

// NOTICE sections, each naming its files and their SPDX licence.
const sections = (notice) =>
  notice
    .split(/^=+\n/m)
    .slice(1)
    .map((text) => ({
      text,
      files: (text.match(/^Files:\s+(.*)$/m)?.[1] ?? '').split(', ').map((f) => f.replace(/\.json$/, '')),
      spdx: text.match(/^SPDX:\s+(.*)$/m)?.[1],
    }));
const grammarNotice = sections(fs.readFileSync(path.join(unpack(TM_GRAMMARS), 'NOTICE'), 'utf8'));
const themeNotice = sections(fs.readFileSync(path.join(unpack(TM_THEMES), 'NOTICE'), 'utf8'));
const licence = new Map(grammarNotice.flatMap((s) => s.files.map((f) => [f, s.spdx])));

const kept = [...grammars.values()].filter((g) => allowed(licence.get(g.name)));
const left = [...grammars.keys()].filter((n) => !allowed(licence.get(n)));
for (const g of kept) {
  const needs = (g.embeddedLangs ?? []).filter((e) => left.includes(e));
  if (needs.length) throw new Error(`${g.name} embeds ${needs}, which is left out`);
}

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(path.join(out, 'grammars'), { recursive: true });
fs.mkdirSync(path.join(out, 'themes'));
const languages = kept
  .sort((a, b) => (a.name < b.name ? -1 : 1))
  .map((g) => {
    fs.writeFileSync(path.join(out, 'grammars', `${g.name}.json`), JSON.stringify(g) + '\n');
    return {
      id: g.name,
      grammar_file: `${g.name}.json`,
      scope_name: g.scopeName,
      display_name: g.displayName,
      aliases: g.aliases ?? [],
      embedded_langs: g.embeddedLangs ?? [],
      embedded_langs_lazy: g.embeddedLangsLazy ?? [],
      inject_to: g.injectTo ?? [],
    };
  });
fs.writeFileSync(path.join(out, 'languages.json'), JSON.stringify({ languages }, null, 1) + '\n');
const themes = [];
for (const name of THEMES) {
  const theme = (await import(path.join(site, `node_modules/@shikijs/themes/dist/${name}.mjs`))).default;
  fs.writeFileSync(path.join(out, 'themes', `${name}.json`), JSON.stringify(theme) + '\n');
  themes.push({ id: name, theme_file: `${name}.json`, display_name: theme.displayName, theme_type: theme.type });
}
fs.writeFileSync(path.join(out, 'themes.json'), JSON.stringify({ themes }, null, 1) + '\n');

const rule = '='.repeat(105) + '\n';
const keptNames = new Set(kept.map((g) => g.name));
const unlicensed = kept.map((g) => g.name).filter((n) => !licence.has(n));
fs.writeFileSync(
  path.join(out, 'NOTICE'),
  [
    `The grammars and themes in this folder are Shiki 3.19's, from ${TM_GRAMMARS} and ${TM_THEMES}.\n` +
      'Each keeps its original licence, set out below as those packages set it out.\n' +
      `Left out for their licence: ${left.join(', ')}.\n` +
      `No licence is stated upstream for: ${unlicensed.join(', ')}.\n`,
    ...grammarNotice.filter((s) => s.files.some((f) => keptNames.has(f))).map((s) => rule + s.text),
    ...themeNotice.filter((s) => s.files.some((f) => THEMES.includes(f))).map((s) => rule + s.text),
  ].join('\n'),
);
console.log(`${kept.length} grammars, left out: ${left.join(', ')}`);

// The corpus's code blocks as the Engine reads them, each with the grammar
// its language resolves to, from tests/highlight.rs.
const list = path.join(tmp, 'code-blocks.json');
execFileSync(
  'cargo',
  ['test', '--locked', '-q', '-p', 'yuanme-render', '--test', 'highlight', '--', '--ignored', '--exact', 'write_code_blocks'],
  { env: { ...process.env, YUANME_CODE_BLOCKS: list }, stdio: 'inherit' },
);
const blocks = JSON.parse(fs.readFileSync(list, 'utf8'));
const { createHighlighter } = await import(path.join(shiki, 'index.mjs'));
const highlighter = await createHighlighter({ langs: [...new Set(blocks.map((b) => b.lang))], themes: THEMES });
// Each style once; a block is its lines, each a flat list of style and
// length in characters.
const styles = [];
const index = new Map();
const style = (light, dark) => {
  const s = JSON.stringify([
    light.color.toLowerCase(),
    Math.max(light.fontStyle ?? 0, 0),
    dark.color.toLowerCase(),
    Math.max(dark.fontStyle ?? 0, 0),
  ]);
  if (!index.has(s)) {
    index.set(s, styles.length);
    styles.push(s);
  }
  return index.get(s);
};
// The themes' own colours first, which is what a newline has.
style({ color: '#4c4f69' }, { color: '#cdd6f4' });
const lines = blocks
  .sort((a, b) => (a.key < b.key ? -1 : 1))
  .map(({ key, lang, code }) => {
    const tokens = highlighter.codeToTokensWithThemes(code, {
      lang,
      themes: { light: THEMES[0], dark: THEMES[1] },
    });
    const runs = tokens.map((line) =>
      line.flatMap((t) => [style(t.variants.light, t.variants.dark), [...t.content].length]),
    );
    return `${JSON.stringify(key)}:${JSON.stringify(runs)}`;
  });
fs.writeFileSync(
  'crates/yuanme-render/tests/shiki/tokens.json',
  `{"styles":[\n${styles.join(',\n')}\n],\n"blocks":{\n${lines.join(',\n')}\n}}\n`,
);
fs.rmSync(tmp, { recursive: true });
console.log(`${blocks.length} code blocks, ${styles.length} styles`);
