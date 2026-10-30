perl -pi -e 's|^route = "/posts/\{slug\}/"$|route = "/blog/{slug}/"|; s|^list_route = "/posts/"$|list_route = "/blog/"|' yuanme.toml
perl -pi -e 's|/posts/|/blog/|g' templates/layouts/base.html templates/home.html templates/404.html
