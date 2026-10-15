# Diagnostics are rustc-shaped JSON with stable, named codes

Every Diagnostic the Engine reports has a code that names its area and problem, such as `org/unsupported-construct`, `math/unknown-label` or `template/unknown-field`. Codes never change meaning and are never reused, and each one has a docs page, `yuanme docs <code>`. With `--json`, a Diagnostic is one line of JSON shaped like rustc's: a level, a message, the code, spans with byte offsets and 1-based lines and columns into the original file, child notes and help, and suggested fixes. Each fix is a set of edits (file, byte range, replacement) with rustc's applicability: `machine-applicable`, `maybe-incorrect`, `has-placeholders` or `unspecified`. The human-readable rendering is included as `rendered`, so a tool can show exactly what a person would see.

Agents fix what they can locate and understand. The surveyed generators fail them in both ways: Eleventy prints nothing for a misspelled variable, and Astro 7's `--json` reports a fatal schema error as prose at `post-5.md:0:0` ([#9](https://github.com/yuann3/yuanme/issues/9)). rustc's format is the one coding agents have seen most, and cargo, clippy and rust-analyzer already turn it into fixes. Named codes can be read in a log without looking anything up, and agents can search for them. Decided in [What exactly is yuanme v1's agent-native surface?](https://github.com/yuann3/yuanme/issues/21).

## Considered Options

- **Numeric codes, like rustc's `E0425`** (rejected): stable, but opaque. A name tells the reader where the problem is before they read the message.
- **LSP's `Diagnostic` shape** (rejected for the CLI): 0-based UTF-16 positions, no suggested edits in the diagnostic itself, and no rendered text. A future language server converts to it.
- **SARIF** (rejected): built for static-analysis result exchange, with far more structure than a build tool needs. It is noisy for an agent to read line by line.

## Consequences

- Codes, field names and the meaning of applicability are public API from v1. A field can be added; nothing can be renamed or removed before 2.0.
- A `machine-applicable` fix must leave the file valid, so the test suite applies every such fix in its fixtures and rebuilds.
- Every code needs a docs page with a failing example and its fix, generated from one registry in `yuanme-diag`, so a code cannot ship without one.
