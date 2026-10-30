# In the one stylesheet, under the tag list's class: a Sidecar would be one
# more request on every Page, and a Post with math and code is at its
# budget of 6 already.
cat >> styles/site.css <<'CSS'

.tag-list a {
  display: inline-block;
  padding: 0.1em 0.7em;
  border-radius: 999px;
  background: var(--surface);
  text-decoration: none;
}
CSS
