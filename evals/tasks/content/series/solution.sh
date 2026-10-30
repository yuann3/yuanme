cat >> yuanme.toml <<'TOML'

[taxonomies.series]
collections = ["posts"]
route = "/series/{term}/"
single = true
TOML
perl -0pi -e 's/\ntags: \[meta\]\n/\ntags: [meta]\nseries: Getting started\n/' content/posts/hello.md
perl -0pi -e 's/\n#\+FILETAGS: :meta:org:\n/\n#+FILETAGS: :meta:org:\n#+SERIES: Getting started\n/' content/posts/hello-org.org
