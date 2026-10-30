{
  printf -- '---\ntitle: A long read\ndate: 2026-03-01\ndescription: Many words about very little.\n---\n\n'
  for i in $(seq 1 120); do
    printf 'Paragraph %s says the same few things again, so that this Post takes a while to read from start to end.\n\n' "$i"
  done
} > content/posts/long-read.md
