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

**Diagnostic**:
An error or warning the Engine reports about an Author's project. It has a stable code, the place in a file it is about, and, when it can, a suggested fix. Errors fail the build; warnings do not, unless strict mode is on.
_Avoid_: lint, log message, error (unqualified: a Diagnostic may be a warning)

### Content

**Source format**:
The markup a piece of content is written in: Markdown or Org. Every Source format maps onto the same metadata model.
_Avoid_: content type, file type

**Org subset**:
The defined set of Org features the Engine renders. Org outside the subset is a build error, never silently dropped or passed through.
_Avoid_: org support (unqualified)

**Org divergence**:
A deliberate, written-down difference between how the Engine reads or renders Org and how the pinned Org release does, such as leaving `snake_case` unsubscripted.
_Avoid_: Parity exception (that compares the Reference site with Astro), quirk

**Label**:
The name an Author gives a heading, equation, Theorem block or other element so that it can be cross-referenced, such as `eq:gauss`. It becomes the element's id, and is unique within its Entry.
_Avoid_: anchor, key, id (that is the HTML attribute a Label becomes)

**Theorem block**:
A block in an Entry's body for a theorem, lemma, definition, proof or another kind the Site declares, numbered or not, and written in the Source format's own block syntax.
_Avoid_: environment (that is LaTeX's word, used here only for math), admonition, callout

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

### Rendering

**Template**:
A file the Engine fills with content to produce a Page or part of one.
_Avoid_: view, theme (a Starter is the ready-made Site)

**Layout**:
A Template that other Templates extend, holding the structure they share.
_Avoid_: base, wrapper

**Component**:
A reusable piece of a Template that takes declared inputs and can carry its own styles and scripts.
_Avoid_: widget, macro (that is how it is written, not what it is), island

**Partial**:
A piece of a Template included as-is, which reads whatever the including Template can see.
_Avoid_: include, snippet

**Route**:
One URL of a Site together with what produces it: an Entry, a list, a Term, or a Template alone.
_Avoid_: path (the URL alone), endpoint

**Page**:
What a visitor loads at one Route. An Entry is content; a Page is that content rendered at a URL.
_Avoid_: document, view

**Router**:
The Engine's built-in client script that moves between Pages of a Site by swapping the new Page into the current document, so what a Site keeps between Pages survives. Browsers without the Navigation API get full page loads instead.
_Avoid_: client-side routing, SPA, transitions (a Route is a URL, not something the Router owns)

**Soft navigation**:
A move between Pages that the Router completes in place, without a full page load.
_Avoid_: client navigation, page transition

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
