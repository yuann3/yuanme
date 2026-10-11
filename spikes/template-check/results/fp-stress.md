| # | Legitimate idiom | SemiStrict render | check | Verdict |
|---|---|---|---|---|
| 1 | `{% if not entry.hero %}none{% else %}{{ entry.hero.src }}{% endif %}` | ok | ``entry.hero` is optional, so `entry.hero.src` reaches through a value that may be undefined; guard with `{% if entry.hero %}`` | **FALSE POSITIVE** |
| 2 | `{% if entry.hero is undefined %}none{% else %}{{ entry.hero.src }}{% endif %}` | ok | ``entry.hero` is optional, so `entry.hero.src` reaches through a value that may be undefined; guard with `{% if entry.hero %}`` | **FALSE POSITIVE** |
| 3 | `{{ entry.hero.src if entry.hero is defined else "" }}` | ok | clean | agree (ok) |
| 4 | `{% if entry.hero and entry.hero.alt %}{{ entry.hero.alt }}{% endif %}` | ok | clean | agree (ok) |
| 5 | `{% if entry.hero %}{% set h = entry.hero %}{{ h.src }}{% endif %}` | ok | clean | agree (ok) |
| 6 | `{% set h = entry.hero\|default(none) %}{% if h %}{{ h.src }}{% endif %}` | ok | clean | agree (ok) |
| 7 | `{{ entry.updated or entry.date }}` | ok | clean | agree (ok) |
| 8 | `{{ entry.series\|default("none")\|upper }}` | ok | clean | agree (ok) |
| 9 | `{% if entry.updated is defined and entry.updated != entry.date %}u{% endif %}` | ok | clean | agree (ok) |
| 10 | `{% if entry.series is defined %}{% set sname = entry.series %}{% else %}{% set sname = "none" %}{% endif %}{{ sname }}` | ok | clean | agree (ok) |
| 11 | `{% for p in collections.posts if p.series is defined %}{{ p.series }}{% endfor %}` | ok | ``p.series` is optional and may be undefined here; wrap it in `{% if p.series %}` or give it `\| default(…)`` | **FALSE POSITIVE** |
| 12 | `{% for p in collections.posts %}{% if not p.updated %}{% continue %}{% endif %}{{ p.updated }}{% endfor %}` | ok | ``p.updated` is optional and may be undefined here; wrap it in `{% if p.updated %}` or give it `\| default(…)`` | **FALSE POSITIVE** |
| 13 | `{% for p in collections.posts %}{{ loop.index }}{{ loop.previtem.title if loop.previtem else "" }}{% endfor %}` | ok | clean | agree (ok) |
| 14 | `{% set ns = namespace(n=0) %}{% for t in entry.tags %}{% set ns.n = ns.n + 1 %}{% endfor %}{{ ns.n }}` | ok | clean | agree (ok) |
| 15 | `{{ entry.tags\|map("upper")\|join(",") }}` | ok | clean | agree (ok) |
| 16 | `{% for k, v in data.nav[0]\|items %}{{ k }}={{ v }}{% endfor %}` | ok | clean | agree (ok) |
| 17 | `{{ entry["title"] }} {{ collections["posts"]\|length }}` | ok | clean | agree (ok) |
| 18 | `{% set key = "title" %}{{ entry[key] }}` | ok | clean (1 warning) | agree (ok) |
| 19 | `{% for g in collections.posts\|groupby("date") %}{{ g.grouper }}:{{ g.list\|length }} {% endfor %}` | ok | clean | agree (ok) |
| 20 | `{{ (collections.posts\|first).title }}` | ok | clean | agree (ok) |
| 21 | `{{ entry.tags[0] if entry.tags else "" }}` | ok | clean | agree (ok) |
| 22 | `{% macro local(x) %}{{ x }}{% endmacro %}{{ local(entry.title) }}` | ok | ``x` is optional and may be undefined here; wrap it in `{% if x %}` or give it `\| default(…)`` | **FALSE POSITIVE** |
| 23 | `{{ entry.title ~ " " ~ entry.reading_time }}` | ok | clean | agree (ok) |
| 24 | `{% filter upper %}{{ entry.title }}{% endfilter %}` | ok | clean | agree (ok) |
| 25 | `{{ taxonomies.tags.terms\|selectattr("count", "gt", 1)\|map(attribute="name")\|join(" ") }}` | ok | clean | agree (ok) |
| 26 | `{% for i in range(entry.tags\|length) %}{{ entry.tags[i] }}{% endfor %}` | ok | clean | agree (ok) |
| 27 | `{{ entry.word_count // 200 }} {{ entry.title\|length > 3 }}` | ok | clean | agree (ok) |
| 28 | `{% with d = entry.updated\|default(entry.date) %}{{ d\|dateformat(format="%Y") }}{% endwith %}` | ok | clean | agree (ok) |
| 29 | `{% for p in collections.posts\|sort_by(["-date"]) %}{{ p.title }}{% break %}{% endfor %}` | ok | clean | agree (ok) |
| 30 | `{% set cls = ["a", "b" if entry.draft else ""]\|select\|join(" ") %}{{ cls }}` | ok | clean | agree (ok) |

30 idioms: 5 false positives, 0 render errors that check did not predict
