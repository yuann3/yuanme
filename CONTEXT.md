# yuanme

A Rust static site generator for programmers' and mathematicians' personal sites and blogs, built to be driven by coding agents as well as by hand.

## Language

### Building a site

**Engine**:
yuanme itself: the general-purpose Rust program anyone uses to build their own Site from Markdown and Org. It runs at build time only; nothing of it runs per request.
_Avoid_: server, backend, framework, runtime

**Site**:
The static files the Engine writes for one Author (HTML, CSS, JS, images, feeds), servable as-is from any static host or CDN.
_Avoid_: app, dist (the folder name, not the concept)

**Author**:
The person who owns a Site, whether they build it by hand or by prompting a coding agent.
_Avoid_: user (ambiguous with Site visitors), customer

**Starter**:
A minimal ready-made Site the Engine can create for a new Author to begin from.
_Avoid_: theme, template (a template is a rendering file)

### Content

**Source format**:
The markup a piece of content is written in: Markdown or Org. Every Source format maps onto the same metadata model.
_Avoid_: content type, file type

**Org subset**:
The defined set of Org features the Engine renders. Org outside the subset is a build error, never silently dropped or passed through.
_Avoid_: org support (unqualified)

**Collection**:
A named group of content files sharing one schema, such as posts, projects or talks. Authors define their own Collections.
_Avoid_: content type, section

**Post**:
An entry in a blog Collection, written in either Source format.
_Avoid_: article, entry

### Proving it works

**Reference site**:
eyuan.me: the first Site built with the Engine. Its parity with its Astro predecessor is the Engine's first acceptance bar.
_Avoid_: demo, example site

**Parity exception**:
A deliberate, written-down difference between the Reference site and its Astro predecessor, such as a fixed Astro bug or a performance-motivated markup change.

**Agent eval**:
The fixed suite of Author prompts run against a fresh coding agent that has only yuanme's docs, scored on success, number of turns and performance budgets.
_Avoid_: benchmark (that is for speed)

**Scale Site**:
A committed, generated Site of about 1,000 Posts in both Source formats, used alongside the Reference site to measure how the Engine behaves at size.
_Avoid_: stress test, large fixture

### Performance budgets

**Budget**:
A numeric limit on one measurable property of a Site or of a build, such as JS bytes per page or warm build time.
_Avoid_: target, SLO

**Gate**:
A Budget enforced in the Engine's own CI on the Reference site, the Starters or the Scale Site. Breaching it fails the change.
_Avoid_: hard cap, check

**Site budget**:
A Budget applied to an Author's Site. Breaching it is a warning unless the Author opts into strict mode, and the Author can override any number.
_Avoid_: limit, quota

**Acknowledged overage**:
An Author's named, per-page allowance above a Site budget. It silences that warning but stays listed in the build report.
_Avoid_: exception (that word belongs to Parity exception), waiver
