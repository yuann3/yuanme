# Runtime templates, not compiled templates

Authors write Jinja-style templates that the Engine loads at runtime. We don't use templates compiled into Rust (askama, maud), even though those are type-checked and marginally faster. Authors install a prebuilt binary and cannot compile templates into it, and requiring a Rust toolchain and a recompile for every template edit would wreck both the Author experience and the agent loop. LLMs also know Jinja syntax extremely well. To recover most of the lost type safety, the Engine validates template variables against the content schema at build time and reports compile-style diagnostics.

## Considered Options

- **Compiled templates (askama/maud)** (rejected): needs a Rust toolchain per Author.
- **Site as a Rust crate depending on the Engine (Leptos-style)** (rejected): same toolchain cost, plus slow compiles.
- **JSX/TSX through an embedded JS engine** (rejected): slower and heavier, and it reintroduces a JS runtime at build time.
