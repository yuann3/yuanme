cat >> yuanme.toml <<'TOML'

[host]
adapter = "netlify"

[[host.headers]]
path = "/*"
headers = { "X-Frame-Options" = "DENY" }

[[host.redirects]]
from = "/posts/hello/"
to = "/blog/hello/"
status = 301
TOML
