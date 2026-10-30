---
title: The CLI
description: Every command and flag, the exit codes, and the JSON that --json prints.
---

`yuanme` runs in the project's folder, the one with `yuanme.toml`, or in the folder `-C DIR` names. Every command that reads the project first writes these docs to `.yuanme/docs/` when they are for another version of yuanme.

## Starting a Site

`yuanme init DIR` creates a Site in `DIR`, or in the current folder without one, from a Starter. It asks nothing, so it runs the same for a person and for a coding agent. The `minimal` Starter, the only one so far, is `yuanme.toml`, a Layout, a home Page, a 404 Page and a stylesheet. `yuanme.toml` starts with two lines that `init` writes:

- `#:schema ./.yuanme/schemas/yuanme.toml.json`, which editors with a TOML language server, such as Taplo or Tombi, read to complete and check keys.
- `yuanme = "X.Y"`, the yuanme release the project needs, which `check` holds every later release to.

It also writes the files a coding agent reads:

- `AGENTS.md`, with a block between `<!-- BEGIN:yuanme -->` and `<!-- END:yuanme -->` that teaches the commands to run, the mistakes to avoid and where the docs are. An `AGENTS.md` of yours keeps its text and gains the block at its end.
- `CLAUDE.md`, which holds `@AGENTS.md` so Claude Code reads it, unless you have one.
- Two skills, `yuanme-verify`, the check, build and look loop, and `yuanme-migrate`, for bringing a site in from Astro, Hugo, Jekyll or Zola. They are in `.agents/skills/` and, for Claude Code, `.claude/skills/`.
- `.gitignore`, with `.yuanme/` and the output directory, or those two lines added to yours.
- `.yuanme/`: these docs, the JSON Schemas and the types for Scripts, which every command rewrites.

`init` never overwrites a file. If any file it would create is already there, it lists them all, exits 2 and changes nothing. A folder that holds other files, such as a Git repository's, is fine.

## Output

What yuanme prints depends on where it goes, never on guessing who runs it:

- **On a terminal**: Diagnostics with the lines they are about, in colour, and a summary.
- **Into a pipe or a file**: one line per Diagnostic, as `path:line:column: error[code]: message`, and no colour.
- **With `--json`**: one JSON object per line on stdout, and nothing else there. Use it from a script or an agent.

Human text always goes to stderr, so stdout holds only what a command prints as its result, such as `yuanme routes`'s table or a schema.

## Exit codes

- `0`: success. Warnings are allowed.
- `1`: the Site has errors.
- `2`: the command can't be carried out as written: a bad flag, no `yuanme.toml`, a name that matches nothing, or a file `init` or `new` would overwrite.
- `101`: yuanme failed, such as when it can't write a file, or crashed.

## `--json`

Each line is one event with a `type`:

- `diagnostic`: one Diagnostic, shaped like rustc's: `level`, `code`, `message`, `spans` with byte offsets and 1-based lines and columns, `children` with help and suggested edits, `docs` and `rendered`. Apply a suggestion whose `applicability` is `machine-applicable` as it is.
- `build-started` and `build-finished`: around `build`, `check` and each build of `dev`; `build-finished` has `ok`, `duration_ms`, `pages`, `changed_urls` and the counts of errors and warnings.
- `server-ready`: `yuanme dev` is serving, at `url`, from the process `pid`.
- `reload`: `yuanme dev` told browsers to reload the Pages at `urls`.
- `browser-console`: a browser on `yuanme dev` logged something or threw an uncaught error.
- `dev-status`: from `yuanme dev status`, whether a dev server runs for the project, and if one does, where, and its last build as `/__yuanme/status` returns it. `yuanme dev stop` prints one once it has stopped.
- `route`: one line of `yuanme routes`.
- `site-created`: what `yuanme init` wrote: the Site's `dir`, its `starter`, the files it `created`, and the files of yours it `updated`.
- `entry-created`: what `yuanme new` wrote.
- `schema`: what `yuanme schema` prints.
- `docs`: a page of these docs.

`yuanme schema events` prints the JSON Schema of every event. A later release may add fields, but renames and removes none before 2.0.

## The dev server

`yuanme dev` builds the Site with its Drafts, serves it on `http://127.0.0.1:4590/`, or the next free port, and builds it again on every save. Read the URL from `server-ready` or `.yuanme/dev.json` rather than assuming the port.

- **Everything is in memory.** Nothing is written to the output directory, which never holds Drafts.
- **Every save rebuilds the whole Site**, in milliseconds, and what changed is shown in the browser without a reload where it can be: a stylesheet is swapped where it is, and with the Router on, a Page is swapped in place, keeping its scroll and what the Site keeps between Pages, and its Sidecar scripts are mounted again, their new code included. Any other change, such as to a Site Script, is a full load. What it serves is what `yuanme build --drafts` makes, but for two things that would slow a save: an image is encoded when a browser first asks for it, and math uses the whole math font rather than a subset. A small dev-only script is added to each HTML file as it is served.
- **An error keeps the last good build served**, with an overlay on the Page that shows its Diagnostics until a save fixes it. They are printed too, as `build` prints them. With `[typescript] check = true`, types are checked after each build that succeeds, and the overlay shows the type errors.
- **What a browser logs reaches the terminal**: console output and uncaught errors, as `browser-console` events with `--json`.
- **`--write DIR`** also writes each build that succeeds to `DIR`, from the project root, for a tool that wants the files. It holds Drafts, so it can't be the output directory. Images are encoded only when a browser asks for them, so it has none of their variants, and its Pages have no dev script.
- **`--poll`** looks at every file every 100 ms, for file systems that never say a file changed, such as some network and container mounts.

One dev server runs for a project. While it runs, `.yuanme/dev.json` says where: its `pid`, `port`, `url`, Engine `version`, when it `started`, and its `log`. Another `yuanme dev` prints its URL, as `server-ready`, and exits 0 rather than starting a second.

- **`yuanme dev --background`** starts it in a process of its own and returns once the first build has finished, with that build's Diagnostics and `server-ready`. It exits 1 if that build failed, but the server runs on, and serves the Site once a save fixes it.
- **`yuanme dev status`** says whether one runs, where, and how its last build went. With `--json`, that is one `dev-status` event.
- **`yuanme dev logs`** prints what the server started with `--background` printed, which is in `.yuanme/dev.log`, in the form it was started with. `--follow` goes on printing until it stops.
- **`yuanme dev stop`** stops it, and returns once it has saved its cache. Stopping when none runs is no error.
- **`GET /__yuanme/status`** on the server returns its last build: `build-finished`'s fields, and its `diagnostics` as `--json` prints them.

## Working with an agent

The loop is: edit, run `yuanme check --json`, and fix each Diagnostic by its code, applying its suggested edits where they are `machine-applicable`. To look at the Site, start `yuanme dev --background --json` once; `yuanme dev status --json` then gives its last build. `yuanme docs CODE` explains a code; `yuanme routes --json` finds which file makes a URL; `yuanme schema` gives the shape of Site config, of each Collection's front matter and of what each Template sees.
