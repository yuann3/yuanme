# What counts as yuanme's public API

From 1.0, yuanme follows semver for the things an Author's project or an agent's tooling depends on, and for nothing else. A breaking change to any of these waits for the next major release:
- **Project files:** the Site config schema, front matter and Org keyword fields, and the Template context, filters and functions.
- **CLI:** commands, flags and exit codes.
- **Machine-readable output:** Diagnostic codes and their meaning (ADR 0012), and the `--json` event schema.
- **Client API:** the Router's events and `yuanme:router` module (ADR 0007), and the `mount` contract (ADR 0013).
- **Site output:** the layout and URLs of a built Site (#10), and the name of the generated scoping attribute (ADR 0006).

Fields may be added to any of these in a minor release. Before 1.0, releases are `0.x`, and a minor bump marks a breaking change, as uv does.

These are **not** public API, so they can change in any release, with the change noted in the release notes:
- **Exact output bytes:** HTML whitespace and attribute order, hashed asset names, image encodes and CSS minification.
- **Engine internals:** the cache format in `.yuanme/cache`, and the Rust library crates. Those are published only so that `cargo install` works, at `0.0.N` with no stability promise.

An Author upgrading within a major release must get a Site that builds and means the same. They need not get the same bytes: encoder and highlighter bumps are expected (#27). A deprecated config key or field keeps working until the next major release. Until then it reports a `config/deprecated` warning with a `machine-applicable` fix, so `check --json` alone is enough to migrate.

Decided in [How is yuanme distributed, versioned and updated?](https://github.com/yuann3/yuanme/issues/37).

## Considered Options

- **Byte-stable output within a major** (rejected): it would freeze every encoder, highlighter and minifier for the life of a major release, against #27's pin-and-bump policy. Authors care that a Page means the same, not that its bytes match.
- **No written policy, breaking changes listed in the changelog** (rejected): this is what Zola and typst do. Agents and CI pin versions, and they need to know which upgrades are safe without reading prose.
- **Semver for the Rust crates too** (rejected): the crates only exist to build the binary. Promising them stability would slow every internal refactor for no Author's benefit.
