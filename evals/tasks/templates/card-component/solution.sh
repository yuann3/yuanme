perl -0pi -e 's/\{% from "components\/post-row.html" import post_row %\}\n/{% from "components\/post-row.html" import post_row %}\n{% from "components\/card.html" import card %}\n/' templates/home.html
perl -0pi -e 's/\{% endblock %\}\n\z/<h2>Projects<\/h2>\n{% for project in collections.projects %}\n  {{ card(href=project.url, title=project.title, description=project.description) }}\n{% endfor %}\n{% endblock %}\n/' templates/home.html
grep -q 'card(href' templates/home.html
# In the one stylesheet: a Sidecar would be one more request on every
# Page, and a Post with math and code is at its budget of 6 already.
cat >> styles/site.css <<'CSS'

.card {
  padding: var(--space);
  border: 1px solid var(--rule);
  border-radius: var(--radius);
}
CSS
