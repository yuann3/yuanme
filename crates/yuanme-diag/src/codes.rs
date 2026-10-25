/// A Diagnostic code and its docs page. Codes never change meaning and are
/// never reused (ADR 0012).
///
/// `non_exhaustive` keeps codes from being created outside this crate, so
/// the `codes!` macro below is the only place a code can come from.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Code {
    pub name: &'static str,
    /// What the code means, as the first paragraph of its docs page.
    pub meaning: &'static str,
    /// A file that triggers the code, shown on the docs page.
    pub example: &'static str,
    /// How to fix it.
    pub fix: &'static str,
}

/// Defining a code here is the only way to create one, and the fields make
/// its docs page part of defining it.
macro_rules! codes {
    ($($id:ident = $name:literal { meaning: $meaning:literal, example: $example:literal, fix: $fix:literal $(,)? })*) => {
        $(pub const $id: Code = Code { name: $name, meaning: $meaning, example: $example, fix: $fix };)*
        /// Every code, for the docs and for the uniqueness test.
        pub const ALL: &[&Code] = &[$(&$id),*];
    };
}

codes! {
    CONFIG_SYNTAX = "config/syntax" {
        meaning: "`yuanme.toml` is not valid TOML, so nothing else in it can be read.",
        example: "[site]\nurl = https://example.com",
        fix: "Fix the TOML at the reported place. Here the string needs quotes: `url = \"https://example.com\"`.",
    }
    CONFIG_UNKNOWN_KEY = "config/unknown-key" {
        meaning: "Site config has a key yuanme doesn't know. Unknown keys are errors, not ignored, so a typo can't silently do nothing. Only `[site.extra]` takes keys of the Author's own.",
        example: "[site]\nurl = \"https://example.com\"\ntitel = \"My site\"",
        fix: "Use the key the help suggests, such as `title`, or move a key of your own into `[site.extra]`.",
    }
    CONFIG_MISSING_KEY = "config/missing-key" {
        meaning: "Site config leaves out a key that has no default.",
        example: "[site]\ntitle = \"My site\"",
        fix: "Add the key. Here every Site needs its address: `url = \"https://example.com\"`.",
    }
    CONFIG_WRONG_TYPE = "config/wrong-type" {
        meaning: "A key in Site config has a value of the wrong TOML type, such as a string where a number goes.",
        example: "[collections.posts]\nroute = \"/blog/{slug}/\"\npaginate = \"10\"",
        fix: "Write the value as the type the message names. Here `paginate = 10`, without quotes.",
    }
    CONFIG_OUT_OF_RANGE = "config/out-of-range" {
        meaning: "A number in Site config is outside the range its key allows.",
        example: "[collections.posts]\nroute = \"/blog/{slug}/\"\nlist_route = \"/blog/\"\npaginate = 0",
        fix: "Use a number inside the range the message names. Here `paginate` is at least 1.",
    }
    CONFIG_INVALID_VALUE = "config/invalid-value" {
        meaning: "A value in Site config has the right type but means nothing to yuanme: a URL that isn't absolute, a Route without its placeholder, an unknown field type, or a name that refers to nothing.",
        example: "[collections.posts]\nroute = \"/blog/\"",
        fix: "Change the value as the message says. Here an Entry's Route needs `{slug}`: `route = \"/blog/{slug}/\"`.",
    }
    CONFIG_ENGINE_VERSION = "config/engine-version" {
        meaning: "The project needs a different version of yuanme than the one running: `yuanme = \"X.Y\"` is the oldest compatible version, with Cargo's caret meaning, so 1.2 accepts any 1.x from 1.2 on and 0.3 accepts only 0.3.x.",
        example: "yuanme = \"9.0\"\n\n[site]\nurl = \"https://example.com\"",
        fix: "Install a version of yuanme that the project accepts, with `yuanme self update X.Y.Z`, or change `yuanme = ` if the project really works with the version you have.",
    }
    CONFIG_DEPRECATED = "config/deprecated" {
        meaning: "A key in Site config has been renamed. The old name keeps working until the next major release.",
        example: "Any key the release notes list as renamed.",
        fix: "Apply the machine-applicable fix, which renames the key; `yuanme check --json` reports it.",
    }
    ENTRY_SYNTAX = "entry/syntax" {
        meaning: "An Entry's front matter, or a Data file, can't be read: it isn't valid YAML, TOML or JSON, its `---` is never closed, or the file isn't UTF-8.",
        example: "---\ntitle: [Pew\n---",
        fix: "Fix the syntax at the reported place. Here the list needs its closing bracket, or the title needs quotes: `title: \"[Pew\"`.",
    }
    ENTRY_TOML_FRONT_MATTER = "entry/toml-front-matter" {
        meaning: "An Entry starts with `+++` TOML front matter. yuanme reads front matter as YAML only, between `---` lines.",
        example: "+++\ntitle = \"Pew\"\n+++",
        fix: "Rewrite it as YAML between `---` lines: `title: Pew`.",
    }
    ENTRY_DUPLICATE_KEY = "entry/duplicate-key" {
        meaning: "A key appears twice in the same mapping of front matter or a Data file. YAML forbids it, and the second value would silently win.",
        example: "---\ntitle: Pew\ntitle: Pew 2\n---",
        fix: "Delete one of them.",
    }
    ENTRY_UNKNOWN_KEY = "entry/unknown-key" {
        meaning: "Front matter, or a Data file with a schema, has a key its Collection or schema doesn't declare. Unknown keys are errors, so a typo can't silently do nothing.",
        example: "---\ntitle: Pew\ndraf: true\n---",
        fix: "Use the key the help suggests, such as `draft`, or declare the field under `[collections.<name>.fields]` in `yuanme.toml`.",
    }
    ENTRY_MISSING_FIELD = "entry/missing-field" {
        meaning: "An Entry, or an object inside one, leaves out a field that is required: `title`, or a declared field with neither `optional = true` nor a `default`.",
        example: "---\ndate: 2025-12-12\n---",
        fix: "Add the field, here `title: Pew`, or make it optional in `yuanme.toml`.",
    }
    ENTRY_WRONG_TYPE = "entry/wrong-type" {
        meaning: "A field has a value of the wrong type, such as a number where text goes. YAML reads `2025` as a number and `yes` as text, so either may need quotes or rewriting.",
        example: "---\ntitle: 2025\n---",
        fix: "Write the value as the type the message names. Here `title: \"2025\"`.",
    }
    ENTRY_OUT_OF_RANGE = "entry/out-of-range" {
        meaning: "A field's value is outside the `min` and `max` its Collection declares: a string's length in characters, or a number.",
        example: "---\ntitle: Pew\norder: 9\n---\n(with order = { type = \"int\", max = 5 })",
        fix: "Use a value inside the range, or change the range in `yuanme.toml`.",
    }
    ENTRY_INVALID_VALUE = "entry/invalid-value" {
        meaning: "A field's value has the right type but isn't allowed: an `enum` value not in its list, a URL that isn't absolute, an image that doesn't exist, or an item repeated in a `unique` list.",
        example: "---\ntitle: Pew\nkind: esay\n---\n(with kind = { type = \"enum\", values = [\"essay\", \"note\"] })",
        fix: "Change the value as the message says. Here `kind: essay`.",
    }
    ENTRY_INVALID_DATE = "entry/invalid-date" {
        meaning: "A `date` or `datetime` field can't be read. Dates are ISO 8601, such as 2025-12-12; datetimes are RFC 3339, such as 2025-12-12T09:30:00+08:00, and one without an offset is in `site.timezone`.",
        example: "---\ntitle: Pew\ndate: Dec 12 2025\n---",
        fix: "Write the date in ISO form, `date: 2025-12-12`; the help suggests it when the date can be read another way.",
    }
    ENTRY_INVALID_SLUG = "entry/invalid-slug" {
        meaning: "A `slug:` override isn't a valid Slug. A Slug is lowercase letters, digits, `-` and `_`, in segments separated by `/`, and an override is used exactly as written.",
        example: "---\ntitle: Pew\nslug: My Post\n---",
        fix: "Use the corrected form the help suggests, here `slug: my-post`.",
    }
    ENTRY_DUPLICATE_SLUG = "entry/duplicate-slug" {
        meaning: "Two Entries in one Collection have the same Slug, so they would be written to the same URL.",
        example: "content/posts/Pew.md and content/posts/pew.org",
        fix: "Rename one of the files, or give one a different `slug:`.",
    }
    ENTRY_TERM_COLLISION = "entry/term-collision" {
        meaning: "Two different Terms of a Taxonomy end up with the same Slug, because a `slugs` override in `yuanme.toml` gives one the Slug of another.",
        example: "slugs = { \"C++\" = \"c\" } in [taxonomies.tags], with Entries tagged both `C++` and `C`",
        fix: "Change the override so every Term keeps its own Slug.",
    }
    ENTRY_OUTPUT_COLLISION = "entry/output-collision" {
        meaning: "Two Routes, or a Route and a Static file, would write the same file of the Site.",
        example: "[collections.posts] route = \"/{slug}/\" and content/pages/about.md, with content/posts/about.md",
        fix: "Change one Collection's `route`, or rename one of the files.",
    }
    ENTRY_DUPLICATE_DATA_FILE = "entry/duplicate-data-file" {
        meaning: "Two Data files share a name, such as `data/nav.toml` and `data/nav.yaml`, so `data.nav` would be ambiguous.",
        example: "data/nav.toml and data/nav.yaml",
        fix: "Delete or rename one of them.",
    }
    ORG_NOT_YET_SUPPORTED = "org/not-yet-supported" {
        meaning: "This build of yuanme can't read Org Entries yet. Org support arrives before the first release, and this code goes away with it.",
        example: "content/posts/proof.org",
        fix: "Write the Entry in Markdown for now, or use a build of yuanme with Org support.",
    }
    MARKDOWN_EMPTY_DIRECTIVE = "markdown/empty-directive" {
        meaning: "A `:::` line opens a block directive with no name. A bare `:::` meant as a closing fence opens a new block instead when it is no longer than an open block's fence, and that block swallows the rest of the file.",
        example: "::::note\n:::tip\nText\n:::\n:::\n(the last line should be `::::`)",
        fix: "Give the opening fence more colons than any block inside it, and close it with the same number, or name the directive.",
    }
    MARKDOWN_NO_EFFECT = "markdown/no-effect" {
        meaning: "Something in a Markdown body is valid but changes nothing in the output, such as a `[title]` on a directive that isn't a Theorem block.",
        example: ":::note[Heads up]\nText\n:::",
        fix: "Remove it, or use a construct that shows it.",
    }
    MARKDOWN_OUT_OF_FUEL = "markdown/out-of-fuel" {
        meaning: "A Markdown body would take longer to parse than yuanme allows for its length, so the build stops instead of hanging. Only pathological input does this, such as hundreds of block directives opened inside each other and never closed.",
        example: ":::a\n:::a\n:::a\n(and so on, a few hundred times)",
        fix: "Close the blocks that start at the reported line, or nest them less deeply.",
    }
    LINK_UNRESOLVED = "link/unresolved" {
        meaning: "A link to another file or a fragment doesn't lead anywhere in the Site: no Entry has that file as its source, or the page has no element with that id.",
        example: "[the setup](setup.md#install), where setup.md has no `## Install` heading",
        fix: "Link to an Entry's file by its path from this file, and to a heading by its id. Links to URLs and to `/` paths are left as they are.",
    }
    TEMPLATE_NOT_FOUND = "template/not-found" {
        meaning: "A Template that a Route renders, or that another Template extends, includes or imports, is not in `templates/`. Each Route has one default Template, such as `posts/entry.html` for an Entry of `posts`, unless Site config names another with `template`.",
        example: "[collections.posts]\nroute = \"/blog/{slug}/\"\n(with no templates/posts/entry.html)",
        fix: "Create the file the message names, or set `template` to a Template that exists. Templates name each other by their path from `templates/`.",
    }
    TEMPLATE_SYNTAX = "template/syntax" {
        meaning: "A Template is not valid Jinja, so no Page that uses it can render.",
        example: "{% if entry.draft %}Draft",
        fix: "Fix the syntax at the reported place. Here the `if` needs its `{% endif %}`.",
    }
    TEMPLATE_RENDER = "template/render" {
        meaning: "A Template failed while rendering a Page: it printed a value that is absent, reached through one, or did something its values don't allow. `{% if entry.hero %}` on an absent field is false, but printing it or reading `entry.hero.src` is an error.",
        example: "<img src=\"{{ entry.hero }}\">\n(on an Entry without `hero`)",
        fix: "Guard the optional value with `{% if %}` or `| default(...)`, or correct the name if it is a typo.",
    }
    TEMPLATE_UNKNOWN_FILTER = "template/unknown-filter" {
        meaning: "A Template calls a filter, test or function that neither Jinja nor yuanme has.",
        example: "{{ entry.date | date_format(format=\"%Y\") }}",
        fix: "Correct the name, here `dateformat`. yuanme adds `sort_by`, `dateformat`, `url`, `absolute_url`, `pluralize`, `attrs`, `markdown` and `org` to Jinja's own.",
    }
    TEMPLATE_BAD_FILTER_ARGUMENT = "template/bad-filter-argument" {
        meaning: "A filter or function got an argument it can't use: a missing or unknown one, a value of the wrong type, or a `dateformat` format it doesn't know.",
        example: "{{ entry.date | dateformat(format=\"iso\") }}",
        fix: "Pass what the message asks for. A `dateformat` format is strftime directives, such as \"%Y-%m-%d\", or \"rfc2822\" or \"rfc3339\".",
    }
    TEMPLATE_UNKNOWN_URL = "template/unknown-url" {
        meaning: "`url()` names a path that is no Route, feed or Static file of the Site, so the link would lead nowhere.",
        example: "<a href=\"{{ url('/abuot/') }}\">About</a>",
        fix: "Use a path the Site has, such as the one the help suggests. Every URL of a Site is declared in Site config.",
    }
    INTERNAL_PANIC = "internal/panic" {
        meaning: "yuanme crashed while processing a file. This is a bug in yuanme, not in the project.",
        example: "Any file can trigger it, if it reaches a bug in a parser or a renderer.",
        fix: "Report it at https://github.com/yuann3/yuanme/issues with the file named in the message. Until it is fixed, changing the construct near the reported place usually avoids it.",
    }
}

/// The areas from #21. A code outside them is a typo.
pub const AREAS: &[&str] = &[
    "config", "entry", "org", "markdown", "math", "link", "template", "asset", "image", "budget",
    "host", "internal",
];
