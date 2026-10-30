cat >> yuanme.toml <<'TOML'

[routes.talks]
path = "/talks/"
template = "talks.html"
TOML
perl -pi -e 's|\{"href": "/about/", "name": "About"\}|{"href": "/talks/", "name": "Talks"}, {"href": "/about/", "name": "About"}|' templates/layouts/base.html
