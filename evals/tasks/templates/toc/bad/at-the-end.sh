perl -0pi -e 's/(  \{% set sections.*?\{% endif %\}\n)(  <div class="prose">\n\{\{ entry.content \}\}\n  <\/div>\n)/$2$1/s' templates/posts/entry.html
grep -q 'endif %}' templates/posts/entry.html
