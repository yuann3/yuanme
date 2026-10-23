"""Aggregate org-features.el output and a math-syntax scan into Markdown tables.

usage: python3 aggregate.py CORPUS_DIR ORG_JSONL > tables.md
"""
import collections, json, os, re, sys

corpus, jsonl = sys.argv[1], sys.argv[2]
manifest = {m["path"]: m for m in json.load(open(os.path.join(corpus, "manifest.json")))}

REFERENCE_SOURCES = ("Org mode manual", "Org mode compact guide", "Org mode test suite", "Worg", "ox-hugo",
                     "org-ref", "org-special-block-extras", "Denote manual", "Howard Abrams", "TEC (tecosaur)")
def is_post(rel):
    return not source_of(rel).startswith(REFERENCE_SOURCES)

def source_of(rel):
    return manifest.get(rel, {}).get("source", rel.split("/")[1])

# --- Org constructs (org-element) ---
occ, files, srcs, posts = collections.Counter(), collections.Counter(), collections.defaultdict(set), collections.Counter()
errors, n_org, n_posts, org_sources = [], 0, 0, set()
for line in open(jsonl):
    r = json.loads(line)
    rel = os.path.relpath(os.path.abspath(r["file"]), os.path.abspath(corpus))
    if "error" in r:
        errors.append((rel, r["error"])); continue
    n_org += 1; org_sources.add(source_of(rel)); post = is_post(rel); n_posts += post
    for k, v in (r["counts"] or {}).items():
        occ[k] += v; files[k] += 1; srcs[k].add(source_of(rel)); posts[k] += post

SKIP = {"table-row", "table-cell", "item", "citation-reference"}
def table(keys, title):
    print(f"\n### {title}\n\n| Construct | Occurrences | Files (of {n_org}) | Posts and notes (of {n_posts}) | Sources (of {len(org_sources)}) |\n|---|---|---|---|---|")
    for k in keys:
        print(f"| `{k}` | {occ[k]:,} | {files[k]} | {posts[k]} | {len(srcs[k])} |")

base = sorted((k for k in occ if ":" not in k and k not in SKIP), key=lambda k: (-files[k], -occ[k]))
table(base, "Element and object types")
groups = collections.OrderedDict([
    ("Keywords (`#+KEY:`)", "keyword:"), ("Affiliated keywords", "affiliated:"),
    ("Headline features", "headline:"), ("Special blocks", "special-block:"),
    ("Source block languages", "src-block:"), ("Inline source blocks", "inline-src-block:"),
    ("Export blocks and snippets", "export-"), ("LaTeX environments", "latex-environment:"),
    ("LaTeX fragments", "latex-fragment:"), ("Link types", "link:"), ("Lists and items", "plain-list:"),
    ("Tables", "table:"), ("Drawers", "drawer:"), ("Properties", "node-property:"),
    ("Macros", "macro:"), ("Timestamps", "timestamp:"), ("Dynamic blocks", "dynamic-block:"),
    ("Citations", "citation:"), ("Footnotes", "footnote-reference:"),
])
for title, prefix in groups.items():
    keys = sorted((k for k in occ if k.startswith(prefix) or (prefix == "plain-list:" and k.startswith("item:"))),
                  key=lambda k: (-files[k], -occ[k]))
    if prefix in ("src-block:", "keyword:", "node-property:", "special-block:", "drawer:", "macro:") and len(keys) > 25:
        rest = keys[25:]; keys = keys[:25]
        print(f"\n_{title}: {len(rest)} rarer values omitted: " + ", ".join(f"`{k.split(':',1)[1]}`" for k in rest[:40]) + ("…" if len(rest) > 40 else "") + "_")
    if keys: table(keys, title)
if errors:
    print("\n### Parse errors\n"); [print(f"- `{f}`: {e}") for f, e in errors]

# --- Math syntax across both formats (regex scan; approximate) ---
MATH = [
    ("inline `$…$`", r"(?<![\\$])\$(?!\$)[^$\n]+?(?<![\\\s])\$(?!\$)"),
    ("inline `\\(…\\)`", r"\\\("),
    ("display `$$…$$`", r"\$\$"),
    ("display `\\[…\\]`", r"\\\["),
    ("`\\begin{equation}`", r"\\begin\{equation\*?\}"),
    ("`\\begin{align}`", r"\\begin\{align\*?\}"),
    ("other `\\begin{…}` (gather, cases, matrix…)", r"\\begin\{(?!equation|align|theorem|lemma|proof|definition|corollary|proposition|remark|example)[a-zA-Z*]+\}"),
    ("`\\label{…}`", r"\\label\{"),
    ("`\\ref` / `\\eqref` / `\\cref`", r"\\(?:eq|c|C|auto)?ref\{"),
    ("`\\tag{…}`", r"\\tag\{"),
    ("macro defs (`\\newcommand`, `\\def`, `\\DeclareMathOperator`)", r"\\(?:re)?newcommand|\\def\\|\\DeclareMathOperator"),
    ("Org theorem-like special blocks", r"(?im)^\s*#\+begin_(?:theorem|lemma|proof|definition|corollary|proposition|remark|thm|lem|defn|prop)\b"),
    ("LaTeX theorem environments", r"\\begin\{(?:theorem|lemma|proof|definition|corollary|proposition|remark)\}"),
    ("Pandoc/Quarto fenced theorem divs (`::: {#thm-…}`)", r"(?m)^:::+\s*\{[^}]*#(?:thm|lem|def|cor|prp|cnj|exm|exr|rem|sol|prf)-"),
    ("Quarto equation ids (`$$ {#eq-…}`)", r"\{#eq-"),
    ("Pandoc/Quarto cross-refs (`@eq-…`, `@thm-…`)", r"@(?:eq|thm|lem|def|cor|prp|fig|tbl|sec)-"),
    ("HTML theorem divs (`<div class=\"theorem\">`)", r"<div[^>]+class=\"[^\"]*(?:theorem|lemma|proof|definition)"),
]
print("\n## Math syntax by format\n\nRegex scan over every corpus file, so it is approximate: `$` amounts in prose can be counted as inline math.\n")
by_fmt = collections.defaultdict(lambda: collections.defaultdict(lambda: [0, 0, set()]))
fmt_files = collections.Counter()
for rel, m in manifest.items():
    p = os.path.join(corpus, rel)
    if not os.path.exists(p): continue
    text = open(p, encoding="utf-8", errors="replace").read()
    fmt = m["format"]; fmt_files[fmt] += 1
    for name, rx in MATH:
        n = len(re.findall(rx, text))
        if n:
            c = by_fmt[fmt][name]; c[0] += n; c[1] += 1; c[2].add(m["source"])
fmts = sorted(fmt_files)
print("| Syntax | " + " | ".join(f"{f}: occ / files (of {fmt_files[f]}) / sources" for f in fmts) + " |")
print("|---|" + "---|" * len(fmts))
for name, _ in MATH:
    cells = []
    for f in fmts:
        o, fi, s = by_fmt[f][name] if name in by_fmt[f] else (0, 0, set())
        cells.append(f"{o:,} / {fi} / {len(s)}" if o else "—")
    print(f"| {name} | " + " | ".join(cells) + " |")
