perl -pi -e 's/\{% if sections \| length > 2 %\}/{% if sections %}/' templates/posts/entry.html
