perl -pi -e 's|^route = "/blog/\{slug\}/"$|route = "/posts/{slug}/"|; s|^list_route = "/blog/"$|list_route = "/posts/"|' yuanme.toml
perl -pi -e 's|/blog/|/posts/|g' templates/layouts/base.html templates/home.html templates/404.html
rm content/posts/hello.md content/posts/hello-org.org
