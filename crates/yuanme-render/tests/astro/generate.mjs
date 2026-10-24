// Writes what the Reference site's Markdown pipeline renders for each
// Reference Post, the bodies tests/corpus.rs must match byte for byte.
// Shiki is off: code highlighting arrives with #53.
//
// Run from the repository root, with the Reference site checked out and
// installed (Astro 5.16.5, @astrojs/markdown-remark 6.3.10):
//   node crates/yuanme-render/tests/astro/generate.mjs ../eyuan.me
import fs from 'node:fs';
import path from 'node:path';

const site = path.resolve(process.argv[2]);
const { createMarkdownProcessor } = await import(
  path.join(site, 'node_modules/@astrojs/markdown-remark/dist/index.js')
);
const processor = await createMarkdownProcessor({ syntaxHighlight: false });
const dir = 'corpus/md/eyuan-me';
for (const name of fs.readdirSync(dir).filter((f) => f.endsWith('.md')).sort()) {
  const text = fs.readFileSync(path.join(dir, name), 'utf8');
  // Astro strips front matter before Markdown sees it.
  const body = text.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, '');
  const { code } = await processor.render(body);
  fs.writeFileSync(path.join('crates/yuanme-render/tests/astro', name.replace(/\.md$/, '.html')), code);
}
