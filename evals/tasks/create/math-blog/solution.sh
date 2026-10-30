rm content/posts/hello.md content/posts/hello-org.org
perl -pi -e 's/^title = "My Site"/title = "Primes and Proofs"/' yuanme.toml
