| Case | Rule | Result | Warning |
|---|---|---|---|
| tag split across Jinja control flow (`split.html`) | insert right after the tag name, never look for the tag end; add a space when the next byte is not whitespace, `>` or `/` | pass |  |
| `>` , `<` and Jinja expressions inside attribute values; `<` in text (`attrgt.html`) | quote-aware tag scan; Jinja delimiters are skipped whole (string-aware) in every state; `<` starts a tag only before a letter | pass |  |
| `<script>`, `<style>`, `<textarea>`, HTML comments, `{# #}`, `{% raw %}` bodies (`rawtext.html`) | bodies copied verbatim; `<script>`/`<style>` themselves are never stamped; a tag inside `{% raw %}` is not stamped | pass |  |
| void and self-closing tags, custom elements (`void.html`) | same rule; the attribute lands before `/` | pass |  |
| inline SVG (camelCase names, `xlink:href`, self-closing, foreignObject) (`svg.html`) | same rule; names keep their case; lower-case `data-*` works in SVG | pass |  |
| `<html>`, `<head>`, `<body>` in a layout; child block content (`layout_child.html`) | `html` and `body` are stamped with the layout's id; `head`, `meta`, `title`, `link`, `base`, `script`, `style`, `noscript`, `template` never; block content takes the child template's id; `:root` is never scoped | pass |  |
| `{% include %}` (`inc_parent.html`) | each file is rewritten when it is loaded, so the included file's tags carry the included file's id | pass |  |
| macro imported from another file, with `caller()` slot content (`macro_user.html`) | macro body tags carry the macro file's id; slot content written at the call site carries the caller's id | pass |  |
| elements from `| safe` strings and safe Values (`safe.html`) | never stamped; `| safe` in text position is a `check` warning; safe Values such as `entry.content` cannot be seen statically and are silent by design | pass | yes |
| dynamic tag names (`dyn.html`) | `<{{ tag }}` and `<h{{ n }}`: stamped after the name expression, with a warning (type selectors cannot be checked); name built by `{% if %}`: not stamped, warning | pass | yes |
| tags inside Jinja string literals and `{% set %}` (`strings.html`) | Jinja delimiters are skipped whole, so strings are never stamped (and print escaped) | pass |  |
| quotes unbalanced inside each Jinja branch (`quotes.html`) | KNOWN LIMIT: the scan loses the tag end; the following tags are not stamped; `check` reports `unterminated tag` | pass | yes |

12 cases, 0 failed.

## Detail

### split.html

source:
```html
<a {% if x %}class="a"{% endif %} href="/">t</a><a{% if x %} class="b"{% endif %}>u</a><a{% if x %}{{ {"class": "c"}|attrs }}{% endif %}>v</a>
```
loaded as:
```html
<a data-y-imqjk3 {% if x %}class="a"{% endif %} href="/">t</a><a data-y-imqjk3 {% if x %} class="b"{% endif %}>u</a><a data-y-imqjk3 {% if x %}{{ {"class": "c"}|attrs }}{% endif %}>v</a>
```
rendered (H = `data-y-imqjk3`):
```html
<a H class="a" href="/">t</a><a H  class="b">u</a><a H  class="c">v</a>
```

### attrgt.html

source:
```html
<a title="a > b" data-x='{{ ">" }}' href="{{ url }}?a=1&b=2">t</a><img alt="<b>not a tag</b>" src="x.png"><input value="{{ '<i>' }}" data-y="{{ "}}" }}"><p>1 < 2 and a <3</p>
```
loaded as:
```html
<a data-y-4xhmsd title="a > b" data-x='{{ ">" }}' href="{{ url }}?a=1&b=2">t</a><img data-y-4xhmsd alt="<b>not a tag</b>" src="x.png"><input data-y-4xhmsd value="{{ '<i>' }}" data-y="{{ "}}" }}"><p data-y-4xhmsd>1 < 2 and a <3</p>
```
rendered (H = `data-y-4xhmsd`):
```html
<a H title="a > b" data-x='&gt;' href="/u?a=1&b=2">t</a><img H alt="<b>not a tag</b>" src="x.png"><input H value="&lt;i&gt;" data-y="}}"><p H>1 < 2 and a <3</p>
```

### rawtext.html

source:
```html
<div><script>if (a < b && c > d) { document.write("<p>x</p>"); }</script>
<style>p > a { color: red } /* <b> */</style>
<!-- <section>commented</section> -->
{% raw %}<aside>{{ not jinja }}</aside>{% endraw %}
{# <footer>jinja comment</footer> #}
<textarea><em>text</em></textarea><ins>after</ins></div>
```
loaded as:
```html
<div data-y-96dlvm><script>if (a < b && c > d) { document.write("<p>x</p>"); }</script>
<style>p > a { color: red } /* <b> */</style>
<!-- <section>commented</section> -->
{% raw %}<aside>{{ not jinja }}</aside>{% endraw %}
{# <footer>jinja comment</footer> #}
<textarea data-y-96dlvm><em>text</em></textarea><ins data-y-96dlvm>after</ins></div>
```
rendered (H = `data-y-96dlvm`):
```html
<div H><script>if (a < b && c > d) { document.write("<p>x</p>"); }</script>
<style>p > a { color: red } /* <b> */</style>
<!-- <section>commented</section> -->
<aside>{{ not jinja }}</aside><textarea H><em>text</em></textarea><ins H>after</ins></div>
```

### void.html

source:
```html
<br><br/><img src="a.png"/><hr /><input disabled><my-el></my-el><x-foo/>
```
loaded as:
```html
<br data-y-2ridpz><br data-y-2ridpz/><img data-y-2ridpz src="a.png"/><hr data-y-2ridpz /><input data-y-2ridpz disabled><my-el data-y-2ridpz></my-el><x-foo data-y-2ridpz/>
```
rendered (H = `data-y-2ridpz`):
```html
<br H><br H/><img H src="a.png"/><hr H /><input H disabled><my-el H></my-el><x-foo H/>
```

### svg.html

source:
```html
<svg viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="g"><stop offset="0"/></linearGradient></defs><path d="M0 0L10 10" fill="url(#g)"/><use xlink:href="#g"/><foreignObject><div>x</div></foreignObject><text>a &lt; b</text></svg>
```
loaded as:
```html
<svg data-y-sbroeo viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><defs data-y-sbroeo><linearGradient data-y-sbroeo id="g"><stop data-y-sbroeo offset="0"/></linearGradient></defs><path data-y-sbroeo d="M0 0L10 10" fill="url(#g)"/><use data-y-sbroeo xlink:href="#g"/><foreignObject data-y-sbroeo><div data-y-sbroeo>x</div></foreignObject><text data-y-sbroeo>a &lt; b</text></svg>
```
rendered (H = `data-y-sbroeo`):
```html
<svg H viewBox="0 0 10 10" xmlns="http://www.w3.org/2000/svg"><defs H><linearGradient H id="g"><stop H offset="0"/></linearGradient></defs><path H d="M0 0L10 10" fill="url(#g)"/><use H xlink:href="#g"/><foreignObject H><div H>x</div></foreignObject><text H>a &lt; b</text></svg>
```

### layout_child.html

source:
```html
{% extends "layout.html" %}
{% block content %}<main>hi</main>{% endblock %}
```
loaded as:
```html
{% extends "layout.html" %}
{% block content %}<main data-y-pdhxh5>hi</main>{% endblock %}
```
rendered (H = `data-y-pdhxh5`, H2 = `data-y-i3z04p` from `layout.html`):
```html
<!doctype html>
<html H2 lang="en"><head><meta charset="utf-8"><title>T <b></title><link rel="x" href="y"><base href="/"></head>
<body H2 class="b"><main H>hi</main><noscript>n</noscript></body></html>
```

### inc_parent.html

source:
```html
<section>{% include "inc_child.html" %}</section>
```
loaded as:
```html
<section data-y-f0dtbi>{% include "inc_child.html" %}</section>
```
rendered (H = `data-y-f0dtbi`, H2 = `data-y-pv25v0` from `inc_child.html`):
```html
<section H><p H2 class="inc">included</p></section>
```

### macro_user.html

source:
```html
{% from "macro_lib.html" import card %}
{% call card(title="T") %}<em>slot</em>{% endcall %}
```
loaded as:
```html
{% from "macro_lib.html" import card %}
{% call card(title="T") %}<em data-y-umprfj>slot</em>{% endcall %}
```
rendered (H = `data-y-umprfj`, H2 = `data-y-u7ycrc` from `macro_lib.html`):
```html
<div H2 class="card"><h2 H2>T</h2><div H2 class="slot"><em H>slot</em></div></div>
```

### safe.html

source:
```html
<div>{{ html|safe }}{{ html }}{{ content }}</div>
```
loaded as:
```html
<div data-y-r9cb3w>{{ html|safe }}{{ html }}{{ content }}</div>
```
rendered (H = `data-y-r9cb3w`):
```html
<div H><b class="k">bold</b>&lt;b class=&quot;k&quot;&gt;bold&lt;/b&gt;<b class="k">bold</b></div>
```
- `safe.html:1:6: warning: `| safe` output is not stamped: sidecar rules reach its elements only through :global()`

### dyn.html

source:
```html
<{{ tag }} class="x">a</{{ tag }}><h{{ level }} id="t">b</h{{ level }}><{% if x %}h1{% else %}h3{% endif %} class="cf">c</{% if x %}h1{% else %}h3{% endif %}><i>after</i>
```
loaded as:
```html
<{{ tag }} data-y-b7yowj class="x">a</{{ tag }}><h{{ level }} data-y-b7yowj id="t">b</h{{ level }}><{% if x %}h1{% else %}h3{% endif %} class="cf">c</{% if x %}h1{% else %}h3{% endif %}><i data-y-b7yowj>after</i>
```
rendered (H = `data-y-b7yowj`):
```html
<section H class="x">a</section><h2 H id="t">b</h2><h1 class="cf">c</h1><i H>after</i>
```
- `dyn.html:1:1: warning: expression glued to a tag name is read as a dynamic tag name: stamped, but `check` cannot match type selectors against it`
- `dyn.html:1:35: warning: expression glued to a tag name is read as a dynamic tag name: stamped, but `check` cannot match type selectors against it`
- `dyn.html:1:72: warning: tag name built by control flow: not stamped (write the tag once per branch)`

### strings.html

source:
```html
{% set s = "<p>in a string</p>" %}{{ "<u>lit</u>" }}<div title="{{ '%}' }}">{{ s }}</div>
```
loaded as:
```html
{% set s = "<p>in a string</p>" %}{{ "<u>lit</u>" }}<div data-y-wiejn3 title="{{ '%}' }}">{{ s }}</div>
```
rendered (H = `data-y-wiejn3`):
```html
&lt;u&gt;lit&lt;/u&gt;<div H title="%}">&lt;p&gt;in a string&lt;/p&gt;</div>
```

### quotes.html

source:
```html
<a {% if x %}title="a{% else %}title="b{% endif %}">t</a><span>after</span>
```
loaded as:
```html
<a data-y-vvy2wx {% if x %}title="a{% else %}title="b{% endif %}">t</a><span>after</span>
```
rendered (H = `data-y-vvy2wx`):
```html
<a H title="a">t</a><span>after</span>
```
- `quotes.html:1:1: warning: unterminated tag (unbalanced quotes across Jinja branches?)`

