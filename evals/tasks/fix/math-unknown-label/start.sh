perl -pi -e 's/and so \[\]\(#eq:odd\) holds\./and so \$\\eqref{eq:odds}\$ holds./' content/posts/hello.md
grep -q 'eqref{eq:odds}' content/posts/hello.md
