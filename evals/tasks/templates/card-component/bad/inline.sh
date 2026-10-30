rm templates/components/card.html
perl -pi -e 's/^\{% from "components\/card.html" import card %\}\n//' templates/home.html
perl -pi -e 's/^  \{\{ card\(href=project.url, title=project.title, description=project.description\) \}\}$/  <article class="card"><h3><a href="{{ project.url }}">{{ project.title }}<\/a><\/h3><\/article>/' templates/home.html
