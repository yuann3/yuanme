perl -pi -e 's|^    <h1>\{\{ entry.title \}\}</h1>$|    <h1>{{ entry.title }}</h1>\n    <p class="lede">{{ entry.summary }}</p>|' templates/posts/entry.html
grep -q 'entry.summary' templates/posts/entry.html
