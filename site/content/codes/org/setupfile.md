---
title: "org/setupfile"
description: "An Org Entry has a `#+SETUPFILE`."
---

An Org Entry has a `#+SETUPFILE`. One Org file is one Entry, so yuanme doesn't read settings from another file.

## Example

```
#+TITLE: Pew
#+SETUPFILE: ../setup.org
```

## Fix

Remove it, and put what it set where yuanme reads it: `#+OPTIONS` in the file itself, LaTeX macros in `[math]` in yuanme.toml, and styles, scripts and analytics in Templates.
