# Static engine, no runtime server, plain JS on the client

yuanme is a build-time static site generator: it writes files that any CDN serves, and nothing of it runs per request. We chose this over a Rust web server (axum/hyper) because a CDN serving prebuilt files beats any origin server on latency, scale and cost, and personal sites have no per-request work to justify one. On the client we ship small hand-written JS, not Rust compiled to WASM, because a WASM runtime costs tens to hundreds of KB plus instantiation time, which works against the performance goal.

## Considered Options

- **Rust origin server** (rejected): slower than a CDN edge cache for static content, and an operational burden for Authors.
- **Static plus a small dynamic server**: deferred, not rejected. It returns only if a specific dynamic feature is ever wanted.
- **WASM client framework (Leptos, Dioxus)** (rejected): payload and startup cost.
