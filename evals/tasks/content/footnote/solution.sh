perl -pi -e 's/^(For every \$n \\ge 1\$.*holds\.)$/$1\[^disq]/' content/posts/hello.md
cat >> content/posts/hello.md <<'MD'

[^disq]: Carl Friedrich Gauss, *Disquisitiones Arithmeticae* (1801).
MD
grep -q '\[\^disq\]$' content/posts/hello.md
