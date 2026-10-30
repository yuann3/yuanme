perl -pi -e 's/^title = "My Site"/title = "Ada Lovelace"/; s/^author = "Your Name"/author = "Ada Lovelace"/; s/^description = "Notes on programming and mathematics."/description = "Notes on mathematics and computing."/' yuanme.toml
rm -r content/projects/example
