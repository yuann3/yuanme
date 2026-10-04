# What do competing generators do, and what does agent-native tooling look like today?

Ticket: [#9](https://github.com/yuann3/yuanme/issues/9). Researched 2026-10-04. Feeds [#21](https://github.com/yuann3/yuanme/issues/21) (the v1 agent-native surface) and, through it, [#22](https://github.com/yuann3/yuanme/issues/22) (the Agent eval suite).

## Question

Survey Hugo, Zola, Astro, Eleventy and Quarto: build speed, content model, templating, Markdown/Org/math support, dev server, distribution, and what Authors and agents struggle with in each. Then survey prior art for agent-native developer tooling as of late 2026: llms.txt, framework docs MCP servers, AGENTS.md and skills conventions, structured machine-readable diagnostics, and how framework teams build agent eval suites (for example Vercel's Next.js evals). Identify gaps yuanme can own.

## Short answer

1. **Nobody combines Org, build-time math with cross-references, and an agent-native surface.** Hugo is the only one of the five that renders Org natively, and since v0.166.0 it denies Org content by default. Hugo also has the only built-in build-time math (`transform.ToMath`, which embeds KaTeX), but KaTeX has no `\label`/`\eqref`. Quarto has the best math authoring (`@eq-`, `#thm-`, proofs), but its HTML math is rendered client-side by MathJax by default, it won't take `.org` input, and it is about 100× slower per page than Hugo. Zola, Astro and Eleventy have neither Org nor math in core. **This is the clearest gap.**
2. **The bar for agent-native tooling is now set by Next.js (16.2/16.3) and Astro 7, not by any SSG.** Next.js ships version-matched docs inside the package, writes a managed block into `AGENTS.md` that points at them, runs an MCP endpoint in its dev server, prints errors with labelled fixes and per-error docs pages, offers skills only for multi-step workflows, and publishes an open agent eval suite (nextjs.org/evals). Astro 7 detects agents and backgrounds its dev server with a lock file, a `status`/`stop`/`logs` CLI and a health endpoint, and it added `--json` logging. Hugo, Zola, Eleventy and Quarto offer agents almost nothing beyond Quarto's `llms-txt` output.
3. **The evidence favours always-on, bundled, version-matched docs over llms.txt and over skills for framework knowledge.** In Vercel's evals an `AGENTS.md` docs index scored 100%, against 53% for a skill the agent had to choose to use and 79% for a skill with explicit instructions. Astro *removed* llms.txt from its docs in April 2026 because those files got under 0.1% of traffic and cost 10% of build time. yuanme should ship docs inside the binary, scaffold an `AGENTS.md` block pointing at them, and treat llms.txt as a cheap **Site output** feature rather than as how agents learn the Engine.
4. **Structured diagnostics are where every SSG is weakest.** In throwaway spikes: Eleventy silently rendered an empty `<h1>` for a misspelled variable and exited 0. Hugo and Zola fail at render time with text-only errors (Zola's Tera 2 at least lists the valid fields). Astro 7's `--json` emits info logs as JSON, but the fatal schema error still comes out as prose on stderr with location `post-5.md:0:0`. None offers an applicable fix. rustc's JSON format (code, spans, `suggested_replacement` with `suggestion_applicability`) is the model to copy.
5. **An agent-native Rust SSG already exists, but it is small and Markdown-only.** [seite](https://github.com/seite-sh/seite) (v0.20.0, 21 stars) has an MCP server, an `AGENTS.md` from `init`, and llms.txt plus per-page `.md` output. It has no Org, only optional `katex-rs` math, Tera 1 templates, and no published eval. yuanme can't claim "has an MCP server" as its difference. Its difference has to be math + Org + checked templates + diagnostics + a published eval.

**Recommendation:** position yuanme as *the generator for programmers and mathematicians whose output an agent can verify*. Own these:

- (a) an Org subset and build-time math with numbering, `\label`/`\ref` and theorems, all as zero-client-JS HTML;
- (b) templates checked against the Collection schema **before** rendering, with rustc-style JSON diagnostics (stable codes, byte and line spans, applicable fixes) and an offline page per error code;
- (c) version-matched docs embedded in the binary (`yuanme docs <topic>`), a managed `AGENTS.md` block written by `init`, and skills only for workflows;
- (d) a dev server that never blocks an agent (lock file, `status`/`stop`/`logs`, a health endpoint), which is cheap and table stakes since Astro 7;
- (e) a public Agent eval in the Next.js style that measures "no docs" against "bundled docs".

Because yuanme is absent from every model's training data, the baseline score will be near zero, and the agent surface carries all of the load. Defer a docs MCP server: Next's own MCP docs tool only *points at* the bundled docs. This matches the map's "MCP later" note.

## Comparison of generators

Versions are current as of 2026-10-04.

| | Hugo | Zola | Astro | Eleventy / Build Awesome | Quarto |
|---|---|---|---|---|---|
| Current release | v0.167.0, 2026-09-28 [[r1]](#r1) | v0.23.6, 2026-09-12 [[z1]](#z1) | 7.3.5, 2026-09-24 (7.4.0-beta.1 on 10-02) [[a1]](#a1) | 3.1.6 stable, 2026-06-02; 4.0.0-alpha.10 canary [[e1]](#e1) | 1.10.18 stable, 2026-07-24; 1.11.5 pre [[q1]](#q1) |
| Language / runtime | Go, single binary | Rust, single binary | Node; Rust compiler and Rust Markdown (Sätteri) since 7.0 [[a2]](#a2) | Node | Deno + Pandoc + Typst + dart-sass + esbuild, bundled |
| Distribution size | 19–20 MB tar.gz (linux-amd64) [[r2]](#r2) | 14–15 MB tar.gz [[z2]](#z2) | 150 MB `node_modules` for a bare 7.3.5 site (measured) | 26 MB `node_modules` for 3.1.6 + syntaxhighlight (measured) | 140 MB (linux tar.gz) to 236 MB (macOS); 719 MB unpacked (measured) [[q2]](#q2) |
| 1k-page cold build (measured, median) | **0.24 s** | 0.42 s | 1.29 s | 0.86 s | 29–49 s for **100** pages |
| Content model | Sections, page bundles, taxonomies; YAML/TOML/JSON front matter; **no user schema** | Sections (`_index.md`) + pages + taxonomies; fixed front matter + `extra`; **no user schema** | Content collections with loaders and **Zod schemas**, type-checked by `astro check` | Data cascade; collections from `tags`; opt-in `eleventyDataSchema` (Zod-compatible) since v3 [[e2]](#e2) | Project types (website/blog/book); YAML front matter validated against Quarto's own option schema |
| Templating | Go `html/template`; system rewritten in v0.146.0 [[r3]](#r3) | Tera v2 since 0.23.0; shortcodes replaced by components ("probably the most breaking version") [[z1]](#z1) | `.astro` components (JSX-like), MDX | Nunjucks, Liquid, WebC, JS and more; Nunjucks fork refactored in 4.0 alpha [[e1]](#e1) | Pandoc templates + Lua filters; shortcodes |
| Markdown | Goldmark | pulldown-cmark | Sätteri (pulldown-cmark fork) default since 7.0; remark/unified opt-in [[a2]](#a2) | markdown-it [[e3]](#e3) | Pandoc |
| Org | Native, but **denied by default since v0.166.0** (XSS via `@@html:@@` and export blocks) [[r4]](#r4) | None (open issue #2790) [[z3]](#z3) | None | None | Not accepted: `quarto render a.org` gives "Can't determine execution engine" (measured) |
| Math | Client MathJax/KaTeX, or build-time `transform.ToMath` (embedded KaTeX, MathML default, macros, `throwOnError`) [[r5]](#r5); no `\label`/`\eqref` (KaTeX #2003 open) [[k1]](#k1) | None built in (issues #1695 and PR #2791 open) [[z3]](#z3) | Plugin only (no built-in math) [[a3]](#a3) | Plugin only (markdown-it plugins) [[e3]](#e3) | Excellent authoring: `{#eq-}`/`@eq-`, `#thm-`/`#lem-`/…, `.proof` [[q3]](#q3); HTML rendering is **client-side MathJax by default** (KaTeX/MathML/WebTeX options) [[q4]](#q4) |
| Dev server | `hugo server`, LiveReload | `zola serve`; rebuilds the whole site on any template change (0.23.5) [[z1]](#z1) | Vite HMR; **agent-aware background mode** (7.0) [[a2]](#a2) | `--serve` with incremental and batched incremental (4.0 alpha) | `quarto preview` |
| Agent features | Contributor `AGENTS.md`/`CLAUDE.md` in the repo only [[r6]](#r6); no docs llms.txt (404) | None found; no docs llms.txt (404) | Docs MCP server (kapa.ai backed); `--json` logging; agent detection + background dev/preview + lock file + `/_astro/status`; **llms.txt removed** [[a4]](#a4)[[a5]](#a5) | None found; no docs llms.txt (404) | `llms-txt: true` site option since 1.9 (`.llms.md` per page + `llms.txt`) [[q5]](#q5); quarto.org serves llms.txt |
| Tailwind without Node | Not since v0.161.0: Tailwind CLI must come from npm [[r7]](#r7) | No Tailwind integration | Needs Node | Needs Node | n/a |

### Measured build speed

Method: one generated corpus of N posts. Each post has YAML or TOML front matter (title, date, two of ten tags) and about 250 words: two headings, a list, a blockquote, a link and one Rust code block. Every tool renders a post page, a blog index and tag pages, with code highlighting on: Chroma classes, Giallo classes, Shiki inline styles, Prism. The output directory is deleted before every run (`hyperfine --prepare`). Machine: Apple M4 Max, 16 cores, 128 GB, macOS. **Caveat:** other research agents were running at the same time (load average 9–19), so absolute numbers are noisy. The 1k ordering held in all three runs. Scripts are in `/private/tmp/ssgbench` (throwaway).

| Pages | Hugo 0.167.0 | Zola 0.23.6 | Eleventy 3.1.6 | Astro 7.3.5 | Quarto 1.10.18 |
|---|---|---|---|---|---|
| 1,000 (10 runs, one batch, median) | 0.235 s (user 0.85, sys 0.59) | 0.418 s (user 0.34, **sys 2.52**) | 0.857 s | 1.288 s | — |
| 10,000 (5–6 runs, median) | 3.79 s | 5.22 s | 10.01 s | 8.43 s | — |
| 100 (3 runs) | — | — | — | — | 29.2 / 48.8 / 38.4 s |

Observations:

- Wall time for the two native tools is dominated by **kernel time spent writing output**. Zola spent 2.5 s of system time for 1k pages and 20 s for 10k, which is more than its wall time because the work is spread across threads. yuanme's output writer (ticket #11/#15) deserves its own measurement.
- Quarto runs Pandoc per document. At 0.3–0.5 s per page it is two orders of magnitude slower than Hugo.
- The Reference site's own Astro 5 baseline is a 1.41–1.55 s warm build for 28 pages (`docs/research/astro-parity-inventory.md` §1).

## Details per generator

### Hugo

- **Status:** very active. Releases v0.164 to v0.167 came between July and September 2026, it is built with Go 1.27, and it has 90k stars [[r1]](#r1).
- **Org:** native renderer, but v0.166.0 made `text/org` "denied by default, as Org mode's export blocks and `@@html:...@@` snippets pass raw HTML through unescaped, making it the same XSS sink as `text/html`". Sites opt back in through `security.allowContent` [[r4]](#r4). The content-formats docs confirm "The Emacs Org Mode content format is denied by default" [[r8]](#r8). Org is treated as a liability, not a feature. That supports yuanme's explicit **Org subset**, where anything outside the subset is a build error and raw-HTML escapes are a deliberate decision.
- **Math:** `transform.ToMath` renders with "an embedded instance of the KaTeX display engine". Its options are `output` (`mathml` by default, or `html`/`htmlAndMathml`, which need the KaTeX stylesheet), `displayMode`, `macros`, `throwOnError` and `strict` [[r5]](#r5). Hugo v0.166.0 bumped the embedded KaTeX to 0.18.4 and asked sites to update their stylesheet link to match [[r4]](#r4), which shows the hidden coupling. KaTeX has no `\label`/`\eqref` (issue #2003, still open) [[k1]](#k1), so numbered, cross-referenced equations are out of reach.
- **Templating:** Go templates. v0.146.0 shipped "a fully refreshed template system" [[r3]](#r3). The most-upvoted open issues are backlinks (#8077), `.gohtml` extensions (#10449), date archives (#448) and custom content-format renderers (#7921) [[r9]](#r9).
- **Errors:** a field typo gives `"…/layouts/page.html:1:35": execute of template failed: … can't evaluate field Titel in type *hugolib.pageState`. It has a location but no suggestion and no machine format, and it surfaces only when a page renders (measured spike). `hugo build --help` has no JSON or format flag.
- **Struggles:** the template lookup order and Go template idioms, and Node is needed again for Tailwind [[r7]](#r7).

### Zola

- **Status:** active. 0.23.0 (2026-08-05) moved to Tera v2 and removed shortcodes in favour of Tera components, calling it "probably the most breaking version of Zola that will happen". 0.22.0 replaced syntect with Giallo, which uses TextMate grammars and VS Code themes (relevant to ticket #5). 0.23.5 rebuilds the whole site in `zola serve` on any template change [[z1]](#z1). The licence is EUPL-1.2, which is one reason not to fork it.
- **Formats:** Markdown only. "Non-md content formats" (#2790, 29 reactions) asks for Typst and others. Math has been requested since 2021 (#1695), and a math PR (#2791) is still open [[z3]](#z3).
- **Errors:** Tera 2 is the most agent-friendly of the text errors I saw. It printed ``Field `titel` is not defined. Available fields: aliases, ancestors, …, title, …`` with a caret under the span (measured). It still has no did-you-mean, no code and no JSON, and it is reported at render time.

### Astro

- **Status:** two majors in 2026: 6.0.0 on 2026-03-10 and 7.0.0 on 2026-06-22 [[a1]](#a1). 7.0 moved to Vite 8, a Rust compiler (`@astrojs/compiler-rs`) and Sätteri, a Rust Markdown pipeline forked from pulldown-cmark. It also made `compressHTML: 'jsx'` the default, removed Astro DB and removed deprecated `astro:transitions` APIs [[a2]](#a2)[[s1]](#s1). Even JS-first generators are moving their hot paths into Rust.
- **Agent features (the most of any SSG):**
  - **Agent detection.** `astro dev` and `astro preview` start detached when an agent is detected. Detection uses the `am-i-vibing` package (`astro/dist/cli/agent.js`, measured), and it is off on Windows since 7.3.4.
  - **Lock files.** `.astro/dev.json` and `.astro/preview.json` record URL, port and PID. There are `astro dev stop | status | logs [--follow]` subcommands, plus `--ignore-lock`.
  - **Health endpoint.** `/_astro/status` returns `{"ok": true}`.
  - **JSON logging.** A global `--json` flag (7.0) [[a2]](#a2)[[a6]](#a6).
  - **Docs MCP server.** It lives at `https://mcp.docs.astro.build/mcp`, uses Streamable HTTP, exposes one tool, `search_astro_docs`, and is backed by kapa.ai [[a4]](#a4).
- **llms.txt removed.** In discussion #13006 (2026-01-02) the Astro docs team wrote that llms.txt requests were "<0.1% of our traffic" and that generating the files "took ~23 seconds… 10% of that build's total". They concluded "llms.txt files are not used by AI tools". PR #13538 removed them on 2026-04-20 and put the effort into the MCP server [[a5]](#a5).
- **Errors:** I introduced `title: 5` into a `z.string()` collection and ran `astro build --json`. stdout carried only three JSON info lines. The fatal error went to stderr as prose, `[InvalidContentEntryDataError] … title: Expected type "string", received "number"`, with a Hint, an error-reference URL and `Location: …/post-5.md:0:0`, then a JS stack (measured). The error-reference URL (one docs page per error) is good prior art. The `0:0` location and the prose-only fatal error are not. Issue #18054 (open) reports that the `glob()` loader "logs the error but continues building and exits with code 0" when a Markdown plugin throws, so "CI treat[s] incomplete output as a successful build" [[a7]](#a7).
- **Struggles:** Astro's own guide says agents "may use older APIs and may not be aware of newer features or recent changes to the framework" [[a4]](#a4). With two majors in four months, stale training data is the main way agents fail on Astro.

### Eleventy, being renamed Build Awesome

- **Status:** 3.1.6 is stable. The 4.0 alphas publish `@awesome.me/buildawesome` alongside `@11ty/eleventy`, the upstream repo moved to `11ty/buildawesome`, the minimum is Node 22.15+, and the Nunjucks fork got a large async refactor [[e1]](#e1). There has been no canary since 2026-07-01.
- **Model:** a flexible data cascade and many template languages. `eleventyDataSchema` (v3+) lets Authors validate data with Zod by throwing [[e2]](#e2), so validation is opt-in, not part of the content model.
- **Errors:** a misspelled `{{ titel }}` rendered `<h1></h1>`, and the build exited 0 (measured). For an agent, this kind of silent failure is the worst case.
- **Struggles:** the most-reacted open issues are about the docs: "11ty documentation refresh" (#3388) and "Docs aren't organized logically/unclear where to start" (#3095) [[e4]](#e4).

### Quarto

- **Status:** 1.10.18 stable, with 1.11 in prerelease (weekly builds in September 2026) [[q1]](#q1).
- **Math:** the strongest feature set for mathematicians: equation labels and references, seven theorem types and proofs [[q3]](#q3). For HTML, though, the choices are MathJax (default), KaTeX, WebTeX, GladTeX, MathML or plain, and there is no build-time HTML+CSS math path [[q4]](#q4). It renders PDFs through Typst or LaTeX.
- **Agent features:** `llms-txt: true` (1.9) writes `.llms.md` next to each HTML page plus an `llms.txt` index. 1.11 fixed unresolved cross-references and garbled math in those files [[q5]](#q5). quarto.org itself serves `llms.txt` with `.llms.md` links (fetched).
- **Struggles:** install size (140–236 MB) and speed (about 0.3–0.5 s per page here). Its most-upvoted open issues are features: glossary (#1697), result folding (#341) and blog page navigation (#3795) [[q6]](#q6).

### seite (agent-native Rust SSG, prior art)

The `seite-sh/seite` repo was created on 2026-02-20 and is at v0.20.0 (2026-09-27), with 21 stars and an MIT licence. It describes itself as "a static site generator where Claude Code is the interface":

- `seite agent` spawns Claude Code.
- `seite mcp` is a stdio MCP server that speaks MCP 2024-11-05 through 2025-11-25 and the stateless 2026-07-28 revision. It has tools such as `seite_build`, `seite_create_content` and `seite_lookup_docs`, and resources such as `seite://docs` and `seite://content`.
- `init` writes `AGENTS.md`, a one-line `CLAUDE.md` and `.mcp.json`.
- Every build emits `llms.txt`, `llms-full.txt` and a `.md` copy of each page.

Its stack is pulldown-cmark, syntect, Tera 1.20, optional `katex-rs` and tiny_http, and it has no Org [[s2]](#s2). It shows that "agent-native" alone is already taken as a label, but not with math, Org, checked templates or published evals.

## Agent-native tooling, late 2026

### llms.txt and Markdown for agents

- **Spec.** llms.txt was proposed by Jeremy Howard on 2024-09-03 and last modified 2026-08-10. It requires an H1, then has an optional blockquote summary, then H2 sections of link lists. An "Optional" section holds links that can be skipped, and the spec recommends serving `.md` versions of pages at the same URL [[l1]](#l1).
- **Evidence of use is weak.** Astro measured under 0.1% of traffic and removed its files [[a5]](#a5). Hugo, Zola and 11ty docs serve no llms.txt (all 404, probed 2026-10-04).
- **Still offered by:**
  - Next.js: `/docs/llms.txt` and `/docs/llms-full.txt`, a `.md` URL for every docs page, and `Accept: text/markdown` negotiation [[n1]](#n1).
  - Quarto [[q5]](#q5).
  - Svelte [[v1]](#v1).
  - The Reference site itself (eyuan.me's `llms.txt`, 5,156 B, parity inventory §1).
- **Content negotiation moved to the CDN.** Cloudflare's "Markdown for Agents" (2026-02-12) converts HTML to Markdown at the edge when a client sends `Accept: text/markdown`, on Pro and higher plans. It adds `x-markdown-tokens` and `Content-Signal` headers and `Vary: Accept` [[c1]](#c1). A static host can't negotiate content on its own, so an SSG can only emit `.md` siblings plus a `<link rel="alternate" type="text/markdown">`.

### Docs MCP servers versus bundled docs

- **Hosted search MCP:** Astro (kapa.ai backed, one search tool) [[a4]](#a4) and Svelte (`list-sections`, `get-documentation`, `svelte-autofixer`, `playground-link`) [[v1]](#v1).
- **Runtime MCP:** the Next.js 16+ dev server exposes `/_next/mcp` with `get_errors`, `get_logs`, `get_routes`, `get_page_metadata`, `get_compilation_issues`, `compile_route` and others. The `next-devtools-mcp` wrapper adds `nextjs_index`/`nextjs_call`, and its `nextjs_docs` tool "point[s] the agent at the version-matched docs bundled… in `node_modules/next/dist/docs/`" rather than serving them [[n2]](#n2).
- **Bundled docs:** Next.js 16.2+ ships its docs inside the package. 16.3+ writes and *restores* a managed `<!-- BEGIN:nextjs-agent-rules -->` block in `AGENTS.md` ("This is NOT the Next.js you know… Read the relevant guide in `node_modules/next/dist/docs/`"), from both `create-next-app` and `next dev` when an agent is detected [[n1]](#n1).
- **Why bundled docs win:** Vercel's 2026-01-27 eval post measured a 53% baseline, 53% for a skill on default behaviour, 79% for a skill with explicit instructions and **100%** for an `AGENTS.md` docs index compressed from about 40 KB to 8 KB. The stated reasons were that there is no "should I look this up?" decision, the content is present every turn, and there are no sequencing problems [[n3]](#n3).
- **MCP spec:** the current version is 2026-07-28 [[m1]](#m1). The Rust SDK is `rmcp` 3.5.0 (2026-09-28) [[m2]](#m2).

### AGENTS.md and Agent Skills

- **AGENTS.md** is "stewarded by the Agentic AI Foundation under the Linux Foundation" and "used by over 60k open-source projects", with 20+ agents supporting it. The nearest file wins, and the user's chat prompt overrides all of them [[g1]](#g1).
- **Agent Skills spec** (agentskills.io) [[g2]](#g2):
  - A skill is a `SKILL.md` with `name` (≤64 characters, lowercase and hyphens, matching its directory) and `description` (≤1024 characters).
  - Optional fields are `license`, `compatibility` (≤500), `metadata` and an experimental `allowed-tools`.
  - Optional directories are `scripts/`, `references/` and `assets/`.
  - Progressive disclosure: about 100 tokens of metadata at startup, a body under 5,000 tokens (under 500 lines) on activation, and references loaded on demand.
  - Skills can be validated with `skills-ref validate`.
- **Skills inside MCP:** AAIF (June 2026) runs a working group on shipping skills as MCP resources, to "ship the manual with the product" [[g3]](#g3).
- **How Next.js splits the work:** "Framework knowledge comes from the bundled docs, not from Skills". Skills cover workflows such as `next-dev-loop` and the Cache Components and Partial Prefetching adoption and optimisation skills, which are distributed through `npx skills add vercel/next.js --skill …` [[n1]](#n1).

### Dev servers for agents

- **Next.js** writes `.next/dev/lock` (PID, port, URL). It reuses an already-running server instead of starting a second one, and it forwards browser console errors to the terminal (`logging.browserToTerminal`) [[n1]](#n1).
- **Astro** does the same with its background mode, lock file, `status`/`stop`/`logs` and `/_astro/status` [[a6]](#a6).
- **Agent detection** in both is an environment heuristic: Astro uses `am-i-vibing`, and Next has `generate-agent-files`. Astro had to exclude Warp ("Hybrid environments like Warp are no longer treated as AI agents") and Windows [[a1]](#a1). Detection is fragile, so explicit flags should always win.

### Structured diagnostics

- **rustc JSON** is the reference format [[d1]](#d1):
  - Each message has `$message_type`, `message`, `code{code, explanation}`, `level`, `spans[]`, `children[]` and `rendered`.
  - Each span has `file_name`, `byte_start/end`, `line_start/end`, `column_start/end`, `is_primary`, `label`, `suggested_replacement` and `suggestion_applicability`.
  - Applicability is one of `MachineApplicable`, `MaybeIncorrect`, `HasPlaceholders` or `Unspecified`.
- **Error prose for agents (Next.js):** a blocking error lists labelled fixes (`[stream]`, `[cache]`, `[block]`) the same way in the terminal and in `next build`. A `Learn more` link goes to a `/docs/messages/<code>` page "written for agents to read", with every fix pattern, the trade-offs and the gotchas [[n1]](#n1).
- **Svelte's `svelte-autofixer`** runs static analysis over agent-written code [[v1]](#v1).
- **Rust crates:** `annotate-snippets` 0.12.16 (rustc's renderer), `miette` 7.6.0, `codespan-reporting` 0.13.1, `ariadne` 0.6.0, `schemars` 1.2.2 for JSON Schema, and `serde-sarif` 0.8.0 [[d2]](#d2).
- **SSG status (measured above):** none of the five gives agents a machine-readable fatal error with a real span.

### Agent eval suites

- **Next.js evals** [[n4]](#n4)[[n5]](#n5):
  - The fixtures live in `vercel/next.js/evals/evals/` (40 entries). Each one is a small app, a `PROMPT.md` written "like a real user would: describe the symptom or goal, not the API", and an `EVAL.ts` of vitest assertions that the agent never sees ("Regex the source, don't run it").
  - Every eval runs in two variants, `baseline` and `agents-md`, which differ by one `AGENTS.md` pointing at the bundled docs. A third `skills` variant can be configured.
  - The runner is `@vercel/agent-eval`. It uses a sandbox (Vercel or local Docker), runs a 12-minute default timeout, and fingerprints results so that only changed evals rerun. CI never runs evals; it only fails on stale ones.
  - Results are published as JSON to nextjs.org/evals, with models at "reasonable effort, not their ceiling" and an `avgCostUsd` computed from transcript tokens.
- **What `@vercel/agent-eval` (2.4.1, 2026-10-02) adds** [[n6]](#n6):
  - Transcript observability in `__agent_eval__/results.json`: `shellCommands`, `filesRead`, `filesModified`, `toolCalls`, `totalTurns`, `errors` and more.
  - Agentic LLM judges (`toSatisfyCriterion` and `toScoreAtLeast` on `environment` or `transcript`), which can be pinned to a fixed agent and model.
  - `transcript.not.toContainText(...)` for "never reached for the old API".
  - Agents covered: Claude Code, Codex, Gemini CLI, Cursor and AI Gateway models.
- **Published results (export 2026-09-25)** [[n7]](#n7). Bundled docs help most where training data is weakest:
  - Claude Sonnet 4.5: 39 → 65.
  - Kimi K2.5: 16 → 45.
  - Claude Sonnet 5: 81 → 97.
  - Frontier models are at the ceiling with or without docs (Claude Opus 5.5 high 97/97; GPT 6 Sol high 97/97).
  - Some runs regress with docs: GPT 5.2 Codex newly failed `agent-041`.
- **For yuanme this inverts.** No model has seen yuanme, so its baseline should sit near 0% and the docs, schemas and diagnostics decide every result. The eval's main job is to tune the agent surface, not to measure how much it adds.

## What Authors and agents struggle with

| Generator | Authors | Agents |
|---|---|---|
| Hugo | Go template idioms and lookup order; Org now opt-in; no backlinks; Node needed for Tailwind | Errors only at render time and only as text; `.Params.x` typos are silent (map lookup); no agent docs |
| Zola | Breaking Tera 2 and shortcode migration; no math, no non-Markdown formats | Text errors (good field lists, no codes or JSON); whole-site rebuild on template change |
| Astro | Major-version churn (6.0 and 7.0 in 2026); plugins needed for math | Stale training data (Astro says so); fatal errors are prose with `:0:0` locations; `glob()` can exit 0 on render errors (#18054) |
| Eleventy | Docs organisation (top issues); many template languages; rename to Build Awesome | Undefined variables render empty and exit 0; no schema by default |
| Quarto | Install size; render speed; client-side math in HTML | Slow edit-verify loop; no Org; `llms-txt` output is its only agent feature |

## Gaps yuanme can own

1. **Org + Markdown on one metadata model, with an explicit Org subset that fails loudly.** Hugo now treats Org as a security risk, and nobody else accepts it. Owning the raw-HTML policy explicitly (for example, `@@html:@@` as a named Parity exception or a diagnostic) answers the reason Hugo gave.
2. **Build-time math with numbering, `\label`/`\ref`, theorems and proofs, and zero client JS.** Today the choice is Quarto's authoring with client-side MathJax, or Hugo's build-time KaTeX without cross-references (#4, #17).
3. **Templates checked against the Collection schema before rendering.** Every SSG surveyed reports template errors at render time, or not at all (Eleventy). ADR 0002 already commits to this. The survey confirms nobody else does it.
4. **A rustc-style JSON diagnostic contract**, covering every failure including fatal ones: a stable code, byte and line spans, related spans, `suggested_replacement` with applicability, and a docs slug. The prose for each code is shipped offline (`yuanme docs E0xxx`), modelled on Next's `/docs/messages` and Astro's error reference. Also a guarantee that the exit code is non-zero whenever any diagnostic is an error (Astro #18054 is the counterexample).
5. **Docs bundled in the binary, matching its version, plus a managed `AGENTS.md` block.** This copies what the evals show works, including the BEGIN/END markers that preserve the Author's own text. Skills are only for workflows, such as "migrate from Astro", "add a Collection" or "adopt Org".
6. **An agent-safe dev server:** a lock file, `status`/`stop`/`logs`, a health endpoint, `--json` events, and reuse of an existing server. It is table stakes now, but no SSG other than Astro has it.
7. **A public Agent eval for an SSG, run with no docs and with bundled docs**, with transcript metrics (turns, tool calls, the commands run) and performance budgets as assertions. No SSG publishes one.
8. **One small binary, zero Node.** Hugo needs npm for Tailwind again, Quarto is 140–236 MB, and Astro is 150 MB of `node_modules`. This supports ADR 0003. yuanme's Tailwind story (#7) is a differentiator, not just parity.
9. **Agent-readable Site output** (`llms.txt`, `.md` siblings, `rel=alternate`) as a cheap opt-in. The evidence that agents use it is weak, so keep the cost near zero.

What **not** to build for v1: a hosted docs-search MCP server, and agent detection that changes behaviour silently. Both are fragile or poorly evidenced. An MCP server can come later as a thin wrapper over the same `--json` commands, which the map already lists as not yet specified.

## ADR check

- No conflict.
- ADR 0002 (runtime Jinja templates validated against the schema) is reinforced: Zola's Tera 2 errors show how much a field list helps, and nobody validates before rendering.
- ADR 0003 (zero Node) is reinforced by Hugo dropping the standalone Tailwind binary in v0.161.0.

## Open questions

- Should `init` write a managed block into `AGENTS.md` (Next.js-style markers, restored on `dev`), or a standalone file that `AGENTS.md` imports? Next.js restoring the block on every `next dev` is an aggressive choice. Decide in #21.
- What should the bundled docs be: a compressed index in `AGENTS.md` (Vercel's 8 KB) plus `yuanme docs <topic>`, or full docs inside the Site's tree? This needs an eval A/B in #22.
- What is the Agent eval's model and harness matrix, and should it reuse `@vercel/agent-eval` (Node, run in CI only, which doesn't violate ADR 0003 for Authors) or use a Rust harness?
- Do we adopt Agent Skills only, or also an MCP-resources mirror once the AAIF working group settles?
- Is Typst-as-a-Source-format (Zola #2790 demand) in scope after v1? It interacts with the math decision (#4).
- Output-writing cost: native SSGs spend most of their wall time in the kernel when writing 10k pages on macOS. Measure write strategies in #11/#15.

## Sources

<a id="r1"></a>[r1] Hugo releases: https://github.com/gohugoio/hugo/releases (v0.167.0, 2026-09-28) · <a id="r2"></a>[r2] Hugo v0.167.0 assets: https://github.com/gohugoio/hugo/releases/tag/v0.167.0 · <a id="r3"></a>[r3] Hugo v0.146.0: https://github.com/gohugoio/hugo/releases/tag/v0.146.0 · <a id="r4"></a>[r4] Hugo v0.166.0 notes (Org denied by default, KaTeX 0.18.4): https://github.com/gohugoio/hugo/releases/tag/v0.166.0 · <a id="r5"></a>[r5] https://gohugo.io/functions/transform/tomath/ and https://gohugo.io/content-management/mathematics/ · <a id="r6"></a>[r6] Hugo commit "Add AGENTS.md and CLAUDE.md" 0fc63fbf (release notes) · <a id="r7"></a>[r7] https://gohugo.io/functions/css/tailwindcss/ · <a id="r8"></a>[r8] https://gohugo.io/content-management/formats/ · <a id="r9"></a>[r9] GitHub search, gohugoio/hugo open issues sorted by reactions (2026-10-04)

<a id="z1"></a>[z1] Zola CHANGELOG: https://github.com/getzola/zola/blob/master/CHANGELOG.md · <a id="z2"></a>[z2] Zola v0.23.6 assets: https://github.com/getzola/zola/releases/tag/v0.23.6 · <a id="z3"></a>[z3] https://github.com/getzola/zola/issues/2790, https://github.com/getzola/zola/issues/1695, https://github.com/getzola/zola/pull/2791

<a id="a1"></a>[a1] Astro releases and CHANGELOG: https://github.com/withastro/astro/blob/main/packages/astro/CHANGELOG.md · <a id="a2"></a>[a2] astro@7.0.0 release notes: https://github.com/withastro/astro/releases/tag/astro%407.0.0 · <a id="a3"></a>[a3] https://docs.astro.build/en/guides/markdown-content/ · <a id="a4"></a>[a4] https://docs.astro.build/en/guides/build-with-ai/ (source `withastro/docs` `src/content/docs/en/guides/build-with-ai.mdx`) · <a id="a5"></a>[a5] https://github.com/withastro/docs/discussions/13006 and https://github.com/withastro/docs/pull/13538 · <a id="a6"></a>[a6] https://docs.astro.build/en/reference/cli-reference/ (`--background`, `--json`, `status`/`logs`) · <a id="a7"></a>[a7] https://github.com/withastro/astro/issues/18054

<a id="e1"></a>[e1] Eleventy releases: https://github.com/11ty/eleventy/releases (v3.1.6; v4.0.0-alpha.8–10) · <a id="e2"></a>[e2] https://www.11ty.dev/docs/data-validate/ · <a id="e3"></a>[e3] https://www.11ty.dev/docs/languages/markdown/ · <a id="e4"></a>[e4] GitHub search, 11ty/buildawesome open issues sorted by reactions

<a id="q1"></a>[q1] https://github.com/quarto-dev/quarto-cli/releases · <a id="q2"></a>[q2] Quarto v1.10.18 assets: https://github.com/quarto-dev/quarto-cli/releases/tag/v1.10.18 · <a id="q3"></a>[q3] https://quarto.org/docs/authoring/cross-references.html · <a id="q4"></a>[q4] https://quarto.org/docs/output-formats/html-basics.html · <a id="q5"></a>[q5] https://github.com/quarto-dev/quarto-cli/blob/main/news/changelog-1.9.md (#13932) and changelog-1.11.md (#14974, #14975); https://quarto.org/llms.txt · <a id="q6"></a>[q6] GitHub search, quarto-dev/quarto-cli open issues sorted by reactions

<a id="k1"></a>[k1] https://github.com/KaTeX/KaTeX/issues/2003 (KaTeX latest v0.19.0, 2026-10-01)

<a id="s1"></a>[s1] Sätteri: https://github.com/bruits/satteri (Cargo workspace incl. `satteri-pulldown-cmark`) · <a id="s2"></a>[s2] seite: https://github.com/seite-sh/seite (README, Cargo.toml, releases)

<a id="n1"></a>[n1] Next.js "How Next.js supports AI coding agents": https://nextjs.org/docs/app/guides/ai-agents (source `vercel/next.js` `docs/01-app/02-guides/ai-agents.mdx`) · <a id="n2"></a>[n2] https://nextjs.org/docs/app/guides/mcp · <a id="n3"></a>[n3] https://vercel.com/blog/agents-md-outperforms-skills-in-our-agent-evals (2026-01-27) · <a id="n4"></a>[n4] https://github.com/vercel/next.js/tree/canary/evals (README, eval.config.json) · <a id="n5"></a>[n5] https://github.com/vercel/next-evals-oss (README) · <a id="n6"></a>[n6] https://github.com/vercel-labs/agent-eval (README); npm `@vercel/agent-eval` 2.4.1 · <a id="n7"></a>[n7] https://raw.githubusercontent.com/vercel/next-evals-oss/main/agent-results.json (exportedAt 2026-09-25)

<a id="v1"></a>[v1] https://svelte.dev/docs/ai/overview · <a id="l1"></a>[l1] https://llmstxt.org/ · <a id="c1"></a>[c1] https://developers.cloudflare.com/fundamentals/reference/markdown-for-agents/ and https://developers.cloudflare.com/changelog/2026-02-12-markdown-for-agents · <a id="m1"></a>[m1] https://github.com/modelcontextprotocol/modelcontextprotocol/releases (2026-07-28) · <a id="m2"></a>[m2] https://crates.io/crates/rmcp · <a id="g1"></a>[g1] https://agents.md/ · <a id="g2"></a>[g2] https://agentskills.io/specification · <a id="g3"></a>[g3] https://aaif.io/blog/skills-over-mcp (2026-06-18) · <a id="d1"></a>[d1] https://doc.rust-lang.org/rustc/json.html · <a id="d2"></a>[d2] crates.io API for annotate-snippets, miette, codespan-reporting, ariadne, schemars, serde-sarif (2026-10-04)

Measured results (marked "measured") come from throwaway spikes in `/private/tmp/ssgbench`, run on 2026-10-04 with Hugo 0.167.0, Zola 0.23.6, Eleventy 3.1.6, Astro 7.3.5 and Quarto 1.10.18.
