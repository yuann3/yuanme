cat >> yuanme.toml <<'TOML'

[routes.archive]
path = "/archive/"
template = "archive.html"
TOML
perl -pi -e 's|\{"href": "/tags/", "name": "Tags"\}|{"href": "/archive/", "name": "Archive"}, {"href": "/tags/", "name": "Tags"}|' templates/layouts/base.html
