cat >> yuanme.toml <<'TOML'

[[host.headers]]
path = "/*"
headers = { "Strict-Transport-Security" = "max-age=31536000; includeSubDomains" }

[[host.redirects]]
from = "/feed.xml"
to = "/rss.xml"
TOML
