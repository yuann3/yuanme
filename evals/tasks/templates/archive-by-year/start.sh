for d in 2024-03-14 2024-11-02 2025-01-20 2025-06-30 2025-12-24; do
  cat > content/posts/notes-$d.md <<MD
---
title: Notes from $d
date: $d
description: What I worked on around $d.
---

Notes.
MD
done
