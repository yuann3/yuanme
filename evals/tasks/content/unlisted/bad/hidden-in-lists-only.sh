perl -pi -e 's/^#\+UNLISTED: true$//' content/posts/hello-org.org
perl -pi -e 's|\{% for post in entries %\}|{% for post in entries if post.slug != "hello-org" %}|' templates/posts/list.html
perl -pi -e 's|\{% for post in collections.posts\[:5\] %\}|{% for post in collections.posts if post.slug != "hello-org" %}|' templates/home.html
