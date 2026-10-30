perl -pi -e 's/^date: 2026-01-01$/date: January 5, 2026/' content/posts/hello.md
grep -q '^date: January 5, 2026$' content/posts/hello.md
