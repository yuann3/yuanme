for i in $(seq -w 1 12); do
  cat > content/posts/note-$i.md <<MD
---
title: Note $i
date: 2026-02-$i
description: A short note, number $i.
tags: [notes]
---

Note $i, about one thing.
MD
done
