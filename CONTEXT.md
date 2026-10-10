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

**Site config**:
The one file at the root of an Author's project that declares the Site: its metadata, Collections, Taxonomies and Engine settings.
_Avoid_: settings, configuration file (unqualified)

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

**Entry**:
One content file in a Collection. It has the fields every Entry shares plus the ones its Collection declares.
_Avoid_: item, document, page (a page is what a visitor loads)

**Post**:
An Entry in a blog Collection, written in either Source format.
_Avoid_: article

**Slug**:
The URL-safe name of an Entry or Term, unique within its Collection or Taxonomy. Two things that would share a Slug are a build error.
_Avoid_: id, permalink

**Draft**:
An Entry that is still being written. It appears while the Author previews the Site and is absent from the published Site.

**Unlisted**:
An Entry that is published and reachable by its URL but left out of every list, Taxonomy, feed and sitemap.
_Avoid_: hidden, private (nothing on a Site is private)

**Taxonomy**:
A named way of grouping Entries by a shared value, such as tags or series. Authors declare their own Taxonomies.
_Avoid_: category (that is one possible Taxonomy), tag system

**Term**:
One value in a Taxonomy, such as the tag `rust`. Spellings with the same Slug are the same Term.
_Avoid_: tag (unless the Taxonomy is tags), label

**Data file**:
A file of structured data that belongs to the Site but is not an Entry, such as a résumé or navigation links.
_Avoid_: content file, config

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
