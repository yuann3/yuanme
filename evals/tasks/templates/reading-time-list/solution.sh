perl -pi -e 's|^  <p>\{\{ post.description \}\}</p>$|  <span class="meta">{{ post.reading_time }} min read</span>\n  <p>{{ post.description }}</p>|' templates/components/post-row.html
grep -q reading_time templates/components/post-row.html
