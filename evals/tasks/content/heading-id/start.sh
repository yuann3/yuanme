perl -pi -e "s/^## A theorem\$/## Gauss's theorem/" content/posts/hello.md
grep -q "^## Gauss's theorem" content/posts/hello.md
