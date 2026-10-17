# Zero-Node toolchain with embedded Rust front-end tools

Building a Site never requires Node.js. The Engine embeds Rust-native front-end tooling (a JS/TS bundler and minifier such as rolldown/oxc, and lightningcss for CSS) and bundles Site code itself, including npm packages from `node_modules`. A package manager is needed only when a Site has a `package.json`, and Bun is the documented choice for that. This keeps the toolchain to one binary for Sites without npm dependencies, while still letting Authors use the npm ecosystem for richer front ends.

## Amendment: Managed tools

Two optional programs may run beside the Engine as Managed tools: the official Tailwind CSS standalone CLI, for Sites whose stylesheets import `tailwindcss`, and TypeScript 7's native `tsc`, for Sites that turn on type checking. Neither is required to build a Site that doesn't use it. The Engine downloads each one on first use, at a version its release pins, and checks it against a SHA-256 checksum the Engine embeds or the upstream release publishes. It caches each tool per user and per version and runs it as a child process. An environment variable can point at a local binary for offline builds and CI. Authors never install them, and neither needs Node. Decided in [What is the front-end pipeline for Authors?](https://github.com/yuann3/yuanme/issues/20).

This doesn't reopen the rejection below. Bun or esbuild would have been a second required runtime for every Site. A Managed tool is optional, pinned and invisible to Authors. It also produces official output, which no Rust reimplementation of Tailwind or TypeScript matches today.

- **Tailwind compiled in an embedded QuickJS** (deferred): it measured +1.4 MB and 35–143 ms per run in a spike, but it brings back the embedded JS runtime that ADR 0002 rejected. The Engine would also have to write Tailwind's class scanner. It stays behind the same Engine module as a later replacement.
- **Hugo removed its support for the standalone Tailwind binary in v0.161** because it now runs Node tools under `node --permission`, which needs a JS entry point. That reason doesn't apply here, because the Engine runs no Node tools.

## Considered Options

- **Shelling out to Bun or esbuild to build front-end code** (rejected): adds a second required runtime and makes builds slower and less hermetic.
- **No npm support at all** (rejected): Authors want packages to build rich front ends.
