perl -pi -e 's/^title = "My Site"/title = "Notes by Sam"/; s/^author = "Your Name"/author = "Sam"/; s/^description = "Notes on programming and mathematics."/description = "Compilers, small tools, and the occasional proof."/' yuanme.toml
rm content/posts/hello.md content/posts/hello-org.org
