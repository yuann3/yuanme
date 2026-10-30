# yuanme

This project is a Site that yuanme builds: a static site generator for Markdown and Org, with math. `yuanme.toml` is the Site config, Entries are Markdown or Org files in `content/`, and Templates are Jinja files in `templates/`.

## Work loop

1. Edit the files.
2. Run `yuanme check --json`. Each line it prints is one JSON event. A `diagnostic` has a `code`, the place in a file, and often a fix: apply a suggestion whose `applicability` is `machine-applicable` as it is, and for any other, read `yuanme docs CODE` and fix what the code names.
3. Repeat until `check` exits 0. Exit 1 means the Site has errors, and 2 that the command itself was wrong.
4. To see the Pages, run `yuanme dev --background --json`. It returns after the first build, with a `server-ready` event that holds the URL: never assume the port. `yuanme dev status --json` says whether a dev server runs and how its last build went, `.yuanme/dev.json` says where it runs, `.yuanme/dev.log` holds what it printed, and `yuanme dev stop` stops it.
5. `yuanme build --json` writes the Site, and `.yuanme/report.md`, which measures each Page against the Site budgets.

More commands that print JSON with `--json`:

- `yuanme routes`: every URL, and the Entry or Template that makes it.
- `yuanme new COLLECTION "Title"`: a new Entry with its required fields filled in.
- `yuanme schema config`, `entry COLLECTION`, `data NAME` or `context KIND`: the JSON Schema of Site config, a Collection's front matter, a Data file, or what a kind of Route's Template sees. `.yuanme/schemas/` holds them too.

## Rules agents get wrong

- An Entry's body is Markdown or Org, never Jinja: `{{ }}` and `{% %}` in a body are text. Logic goes in Templates.
- Org outside the Org subset is an error, never dropped. `yuanme docs org` lists the subset.
- Label an equation with `\label{eq:name}` inside its math, and cite it with `\eqref{eq:name}`, in both Source formats. A link with no text to a Label, `[](#thm:name)` in Markdown or `[[thm:name]]` in Org, shows the target's number.
- A key that Site config or a Collection doesn't declare is an error, and the help names the key meant. Read the schema rather than guessing.
- Never edit `.yuanme/`: yuanme rewrites everything in it.
- A Page over a Site budget is a warning. Make the Page smaller before you acknowledge it in `[budgets.overages]`.

## Docs

These pages are for this version of yuanme. Read the one for a task before you start it; `yuanme docs NAME` prints the same.

