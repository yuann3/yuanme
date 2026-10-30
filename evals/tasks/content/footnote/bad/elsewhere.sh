perl -pi -e 's/\[\^disq\]$//' content/posts/hello.md
perl -pi -e 's/^(The sum of the first \$n\$ odd numbers is a square:)$/$1\[^disq]/' content/posts/hello.md
