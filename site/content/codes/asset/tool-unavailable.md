---
title: "asset/tool-unavailable"
description: "yuanme can't get a Managed tool the Site needs, such as the Tailwind CLI for a stylesheet that imports Tailwind."
---

yuanme can't get a Managed tool the Site needs, such as the Tailwind CLI for a stylesheet that imports Tailwind. yuanme downloads it with `curl` on first use, checks its SHA-256 and keeps it in a per-user cache, so the download failed, perhaps offline, its checksum didn't match, the version isn't released for this platform, or the environment variable that names a local binary, such as `YUANME_TAILWIND_BIN`, names no file.

## Example

```
@import "tailwindcss";
(on a first build without internet)
```

## Fix

Build once with internet, or set `YUANME_TAILWIND_BIN` to a Tailwind CLI binary downloaded from its GitHub release. For a checksum that doesn't match, build again; if it persists, report it. Check that `[tailwind] version` is a released version.
