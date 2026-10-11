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
H2<H2aH2 H2dH2aH2tH2aH2-H2yH2-H2iH2mH2qH2jH2kH23H2 H2cH2lH2aH2sH2sH2=H2"H2aH2"H2 H2hH2rH2eH2fH2=H2"H2/H2"H2>H2tH2<H2/H2aH2>H2<H2aH2 H2dH2aH2tH2aH2-H2yH2-H2iH2mH2qH2jH2kH23H2 H2 H2cH2lH2aH2sH2sH2=H2"H2bH2"H2>H2uH2<H2/H2aH2>H2<H2aH2 H2dH2aH2tH2aH2-H2yH2-H2iH2mH2qH2jH2kH23H2 H2 H2cH2lH2aH2sH2sH2=H2"H2cH2"H2>H2vH2<H2/H2aH2>H2
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
H2<H2aH2 H2dH2aH2tH2aH2-H2yH2-H24H2xH2hH2mH2sH2dH2 H2tH2iH2tH2lH2eH2=H2"H2aH2 H2>H2 H2bH2"H2 H2dH2aH2tH2aH2-H2xH2=H2'H2&H2gH2tH2;H2'H2 H2hH2rH2eH2fH2=H2"H2/H2uH2?H2aH2=H21H2&H2bH2=H22H2"H2>H2tH2<H2/H2aH2>H2<H2iH2mH2gH2 H2dH2aH2tH2aH2-H2yH2-H24H2xH2hH2mH2sH2dH2 H2aH2lH2tH2=H2"H2<H2bH2>H2nH2oH2tH2 H2aH2 H2tH2aH2gH2<H2/H2bH2>H2"H2 H2sH2rH2cH2=H2"H2xH2.H2pH2nH2gH2"H2>H2<H2iH2nH2pH2uH2tH2 H2dH2aH2tH2aH2-H2yH2-H24H2xH2hH2mH2sH2dH2 H2vH2aH2lH2uH2eH2=H2"H2&H2lH2tH2;H2iH2&H2gH2tH2;H2"H2 H2dH2aH2tH2aH2-H2yH2=H2"H2}H2}H2"H2>H2<H2pH2 H2dH2aH2tH2aH2-H2yH2-H24H2xH2hH2mH2sH2dH2>H21H2 H2<H2 H22H2 H2aH2nH2dH2 H2aH2 H2<H23H2<H2/H2pH2>H2
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
H2<H2dH2iH2vH2 H2dH2aH2tH2aH2-H2yH2-H29H26H2dH2lH2vH2mH2>H2<H2sH2cH2rH2iH2pH2tH2>H2iH2fH2 H2(H2aH2 H2<H2 H2bH2 H2&H2&H2 H2cH2 H2>H2 H2dH2)H2 H2{H2 H2dH2oH2cH2uH2mH2eH2nH2tH2.H2wH2rH2iH2tH2eH2(H2"H2<H2pH2>H2xH2<H2/H2pH2>H2"H2)H2;H2 H2}H2<H2/H2sH2cH2rH2iH2pH2tH2>H2
H2<H2sH2tH2yH2lH2eH2>H2pH2 H2>H2 H2aH2 H2{H2 H2cH2oH2lH2oH2rH2:H2 H2rH2eH2dH2 H2}H2 H2/H2*H2 H2<H2bH2>H2 H2*H2/H2<H2/H2sH2tH2yH2lH2eH2>H2
H2<H2!H2-H2-H2 H2<H2sH2eH2cH2tH2iH2oH2nH2>H2cH2oH2mH2mH2eH2nH2tH2eH2dH2<H2/H2sH2eH2cH2tH2iH2oH2nH2>H2 H2-H2-H2>H2
H2<H2aH2sH2iH2dH2eH2>H2{H2{H2 H2nH2oH2tH2 H2jH2iH2nH2jH2aH2 H2}H2}H2<H2/H2aH2sH2iH2dH2eH2>H2<H2tH2eH2xH2tH2aH2rH2eH2aH2 H2dH2aH2tH2aH2-H2yH2-H29H26H2dH2lH2vH2mH2>H2<H2eH2mH2>H2tH2eH2xH2tH2<H2/H2eH2mH2>H2<H2/H2tH2eH2xH2tH2aH2rH2eH2aH2>H2<H2iH2nH2sH2 H2dH2aH2tH2aH2-H2yH2-H29H26H2dH2lH2vH2mH2>H2aH2fH2tH2eH2rH2<H2/H2iH2nH2sH2>H2<H2/H2dH2iH2vH2>H2
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
H2<H2bH2rH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2>H2<H2bH2rH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2/H2>H2<H2iH2mH2gH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2 H2sH2rH2cH2=H2"H2aH2.H2pH2nH2gH2"H2/H2>H2<H2hH2rH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2 H2/H2>H2<H2iH2nH2pH2uH2tH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2 H2dH2iH2sH2aH2bH2lH2eH2dH2>H2<H2mH2yH2-H2eH2lH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2>H2<H2/H2mH2yH2-H2eH2lH2>H2<H2xH2-H2fH2oH2oH2 H2dH2aH2tH2aH2-H2yH2-H22H2rH2iH2dH2pH2zH2/H2>H2
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
H2<H2sH2vH2gH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2 H2vH2iH2eH2wH2BH2oH2xH2=H2"H20H2 H20H2 H21H20H2 H21H20H2"H2 H2xH2mH2lH2nH2sH2=H2"H2hH2tH2tH2pH2:H2/H2/H2wH2wH2wH2.H2wH23H2.H2oH2rH2gH2/H22H20H20H20H2/H2sH2vH2gH2"H2>H2<H2dH2eH2fH2sH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2>H2<H2lH2iH2nH2eH2aH2rH2GH2rH2aH2dH2iH2eH2nH2tH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2 H2iH2dH2=H2"H2gH2"H2>H2<H2sH2tH2oH2pH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2 H2oH2fH2fH2sH2eH2tH2=H2"H20H2"H2/H2>H2<H2/H2lH2iH2nH2eH2aH2rH2GH2rH2aH2dH2iH2eH2nH2tH2>H2<H2/H2dH2eH2fH2sH2>H2<H2pH2aH2tH2hH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2 H2dH2=H2"H2MH20H2 H20H2LH21H20H2 H21H20H2"H2 H2fH2iH2lH2lH2=H2"H2uH2rH2lH2(H2#H2gH2)H2"H2/H2>H2<H2uH2sH2eH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2 H2xH2lH2iH2nH2kH2:H2hH2rH2eH2fH2=H2"H2#H2gH2"H2/H2>H2<H2fH2oH2rH2eH2iH2gH2nH2OH2bH2jH2eH2cH2tH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2>H2<H2dH2iH2vH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2>H2xH2<H2/H2dH2iH2vH2>H2<H2/H2fH2oH2rH2eH2iH2gH2nH2OH2bH2jH2eH2cH2tH2>H2<H2tH2eH2xH2tH2 H2dH2aH2tH2aH2-H2yH2-H2sH2bH2rH2oH2eH2oH2>H2aH2 H2&H2lH2tH2;H2 H2bH2<H2/H2tH2eH2xH2tH2>H2<H2/H2sH2vH2gH2>H2
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
H2<H2dH2iH2vH2 H2dH2aH2tH2aH2-H2yH2-H2rH29H2cH2bH23H2wH2>H2<H2bH2 H2cH2lH2aH2sH2sH2=H2"H2kH2"H2>H2bH2oH2lH2dH2<H2/H2bH2>H2&H2lH2tH2;H2bH2 H2cH2lH2aH2sH2sH2=H2&H2qH2uH2oH2tH2;H2kH2&H2qH2uH2oH2tH2;H2&H2gH2tH2;H2bH2oH2lH2dH2&H2lH2tH2;H2/H2bH2&H2gH2tH2;H2<H2bH2 H2cH2lH2aH2sH2sH2=H2"H2kH2"H2>H2bH2oH2lH2dH2<H2/H2bH2>H2<H2/H2dH2iH2vH2>H2
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
H2<H2sH2eH2cH2tH2iH2oH2nH2 H2dH2aH2tH2aH2-H2yH2-H2bH27H2yH2oH2wH2jH2 H2cH2lH2aH2sH2sH2=H2"H2xH2"H2>H2aH2<H2/H2sH2eH2cH2tH2iH2oH2nH2>H2<H2hH22H2 H2dH2aH2tH2aH2-H2yH2-H2bH27H2yH2oH2wH2jH2 H2iH2dH2=H2"H2tH2"H2>H2bH2<H2/H2hH22H2>H2<H2hH21H2 H2cH2lH2aH2sH2sH2=H2"H2cH2fH2"H2>H2cH2<H2/H2hH21H2>H2<H2iH2 H2dH2aH2tH2aH2-H2yH2-H2bH27H2yH2oH2wH2jH2>H2aH2fH2tH2eH2rH2<H2/H2iH2>H2
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
H2&H2lH2tH2;H2uH2&H2gH2tH2;H2lH2iH2tH2&H2lH2tH2;H2/H2uH2&H2gH2tH2;H2<H2dH2iH2vH2 H2dH2aH2tH2aH2-H2yH2-H2wH2iH2eH2jH2nH23H2 H2tH2iH2tH2lH2eH2=H2"H2%H2}H2"H2>H2&H2lH2tH2;H2pH2&H2gH2tH2;H2iH2nH2 H2aH2 H2sH2tH2rH2iH2nH2gH2&H2lH2tH2;H2/H2pH2&H2gH2tH2;H2<H2/H2dH2iH2vH2>H2
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
H2<H2aH2 H2dH2aH2tH2aH2-H2yH2-H2vH2vH2yH22H2wH2xH2 H2tH2iH2tH2lH2eH2=H2"H2aH2"H2>H2tH2<H2/H2aH2>H2<H2sH2pH2aH2nH2>H2aH2fH2tH2eH2rH2<H2/H2sH2pH2aH2nH2>H2
```
- `quotes.html:1:1: warning: unterminated tag (unbalanced quotes across Jinja branches?)`

