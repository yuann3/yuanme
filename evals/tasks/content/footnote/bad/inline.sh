perl -pi -e 's/\[\^disq\]$/ (Gauss, *Disquisitiones Arithmeticae*, 1801)/' content/posts/hello.md
perl -0pi -e 's/\n\[\^disq\]: .*\n//' content/posts/hello.md
