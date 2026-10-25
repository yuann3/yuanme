---
title: Site config
description: yuanme.toml, the one file that declares the Site, with every key it takes.
---

`yuanme.toml`, at the root of the project, is the Site config. Only `[site]` and its `url` are required:

```toml check
[site]
url = "https://example.com"
```

Unknown keys are errors, not ignored, so a typo can't silently do nothing. The help names the key that was probably meant:

```toml fail config/unknown-key
[site]
url = "https://example.com"
titel = "Example"
```

Keys of your own go in `[site.extra]`, which Templates see as `site.extra`:

```toml check
[site]
url = "https://example.com"
title = "Example"

[site.extra]
github = "https://github.com/example"
```

`yuanme schema config` prints Site config's JSON Schema, which `check`, `build` and the other commands also write to `.yuanme/schemas/yuanme.toml.json`. An editor with a TOML language server, such as Taplo or Tombi, reads it from a first line of:

```toml
#:schema ./.yuanme/schemas/yuanme.toml.json
```

Collections and their fields have their own page, `yuanme docs collections`.
