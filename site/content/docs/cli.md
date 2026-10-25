---
title: "The CLI"
description: "Every command and flag, the exit codes, and the JSON that --json prints."
---

`yuanme` runs in the project's folder, the one with `yuanme.toml`, or in the folder `-C DIR` names. Every command that reads the project first writes these docs to `.yuanme/docs/` when they are for another version of yuanme.

## Output

What yuanme prints depends on where it goes, never on guessing who runs it:

- **On a terminal**: Diagnostics with the lines they are about, in colour, and a summary.
- **Into a pipe or a file**: one line per Diagnostic, as `path:line:column: error[code]: message`, and no colour.
- **With `--json`**: one JSON object per line on stdout, and nothing else there. Use it from a script or an agent.

Human text always goes to stderr, so stdout holds only what a command prints as its result, such as `yuanme routes`'s table or a schema.

## Exit codes

- `0`: success. Warnings are allowed.
- `1`: the Site has errors.
- `2`: the command can't be carried out as written: a bad flag, no `yuanme.toml`, or a name that matches nothing.
- `101`: yuanme failed, such as when it can't write a file, or crashed.

## `--json`

Each line is one event with a `type`:

- `diagnostic`: one Diagnostic, shaped like rustc's: `level`, `code`, `message`, `spans` with byte offsets and 1-based lines and columns, `children` with help and suggested edits, `docs` and `rendered`. Apply a suggestion whose `applicability` is `machine-applicable` as it is.
- `build-started` and `build-finished`: around `build`, `check` and each build of `dev`; `build-finished` has `ok`, `duration_ms`, `pages`, `changed_urls` and the counts of errors and warnings.
- `server-ready`: `yuanme dev` is serving, at `url`, from the process `pid`.
- `reload`: `yuanme dev` told browsers to reload the Pages at `urls`.
- `browser-console`: a browser on `yuanme dev` logged something or threw an uncaught error.
- `route`: one line of `yuanme routes`.
- `entry-created`: what `yuanme new` wrote.
- `schema`: what `yuanme schema` prints.
- `docs`: a page of these docs.

`yuanme schema events` prints the JSON Schema of every event. A later release may add fields, but renames and removes none before 2.0.

## The dev server

`yuanme dev` builds the Site with its Drafts, serves it on `http://127.0.0.1:4590/`, or the next free port, and builds it again on every save. Read the URL from `server-ready` rather than assuming the port.

- **Everything is in memory.** Nothing is written to the output directory, which never holds Drafts.
- **Every save rebuilds the whole Site**, in milliseconds, and the Pages whose files changed are reloaded. What it serves is what `yuanme build --drafts` makes, but for two things that would slow a save: an image is encoded when a browser first asks for it, and math uses the whole math font rather than a subset.
- **An error keeps the last good build served.** Its Diagnostics are printed, as `build` prints them, until a save fixes it.
- **`--poll`** looks at every file every 100 ms, for file systems that never say a file changed, such as some network and container mounts.

## Working with an agent

The loop is: edit, run `yuanme check --json`, and fix each Diagnostic by its code, applying its suggested edits where they are `machine-applicable`. `yuanme docs CODE` explains a code; `yuanme routes --json` finds which file makes a URL; `yuanme schema` gives the shape of Site config, of each Collection's front matter and of what each Template sees.

## Reference

### Global options

- `--json`: Print NDJSON events on stdout, one per line. Human text, if any, goes to stderr.
- `-C DIR`: Run as if yuanme was started in DIR.
- `--color COLOR`: Colour human output. `auto` colours it when stdout is a terminal. One of `auto`, `always`, `never`. Defaults to `auto`.
- `-q, --quiet`: Print only errors.
- `-h, --help`: Print help.
- `-V, --version`: Print version.

### `yuanme build`

Build the Site into the output directory.

```text
yuanme build [OPTIONS]
```

- `--drafts`: Include Drafts.
- `--strict`: Make every warning an error.
- `--strict-budgets`: Make every warning about a Site budget an error, as `budgets.strict` does.
- `--clean`: Empty the output directory first, files yuanme didn't write included.
- `--out DIR`: The output directory, from the project root. By default `dist`, or `.vercel/output` with the vercel adapter.

### `yuanme check`

Report every Diagnostic `build` would, rendering every Page in memory and writing none.

```text
yuanme check [OPTIONS]
```

- `--drafts`: Include Drafts.
- `--strict`: Make every warning an error.

### `yuanme dev`

Serve the Site with its Drafts from memory, and rebuild it on every change.

```text
yuanme dev [OPTIONS]
```

- `--port PORT`: The port on 127.0.0.1. By default 4590, or the next free one.
- `--poll`: Look for changes every 100 ms instead of being told of them, for file systems that tell nothing, such as some network and container mounts.

### `yuanme new`

Create an Entry whose required fields are filled in, and print its path and Route.

```text
yuanme new [OPTIONS] <COLLECTION> <TITLE>
```

- `COLLECTION`: The Collection, by its name in yuanme.toml.
- `TITLE`: The Entry's title, from which its Slug and file name come.
- `--format FORMAT`: The Source format. One of `md`, `org`. Defaults to `md`.

### `yuanme routes`

List every Route and what produces it.

```text
yuanme routes [OPTIONS]
```

- `--drafts`: Include Drafts.

### `yuanme schema`

Print a JSON Schema.

```text
yuanme schema [OPTIONS] <COMMAND>
```

### `yuanme schema config`

Site config, yuanme.toml.

```text
yuanme schema config [OPTIONS]
```

### `yuanme schema entry`

The front matter of a Collection's Entries.

```text
yuanme schema entry [OPTIONS] <COLLECTION>
```

### `yuanme schema data`

A Data file that `[data.NAME]` declares.

```text
yuanme schema data [OPTIONS] <NAME>
```

### `yuanme schema context`

What a kind of Route's Template sees: entry, list, term, index, template or feed.

```text
yuanme schema context [OPTIONS] <KIND>
```

### `yuanme schema events`

The events `--json` prints.

```text
yuanme schema events [OPTIONS]
```

### `yuanme docs`

Print a docs page by topic or Diagnostic code; with none, the index.

```text
yuanme docs [OPTIONS] [TOPIC]
```

- `TOPIC`: A topic, such as `templates`, or a code, such as `entry/unknown-key`.
