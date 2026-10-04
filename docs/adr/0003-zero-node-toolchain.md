# Zero-Node toolchain with embedded Rust front-end tools

Building a Site never requires Node.js. The Engine embeds Rust-native front-end tooling (a JS/TS bundler and minifier such as rolldown/oxc, and lightningcss for CSS) and bundles Site code itself, including npm packages from `node_modules`. A package manager is needed only when a Site has a `package.json`, and Bun is the documented choice for that. This keeps the toolchain to one binary for Sites without npm dependencies, while still letting Authors use the npm ecosystem for richer front ends.

## Considered Options

- **Shelling out to Bun or esbuild to build front-end code** (rejected): adds a second required runtime and makes builds slower and less hermetic.
- **No npm support at all** (rejected): Authors want packages to build rich front ends.
