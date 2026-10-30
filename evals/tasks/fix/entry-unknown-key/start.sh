perl -pi -e 's/^tags: \[meta\]$/tag: [meta]/' content/posts/hello.md
grep -q '^tag: \[meta\]$' content/posts/hello.md
