perl -pi -e 's/\{% for post in collections.posts %\}/{% for post in collections.posts | sort_by(["date"]) %}/' templates/archive.html
