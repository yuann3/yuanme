| Construct | Template | SemiStrict | Strict |
|---|---|---|---|
| if on absent optional | `{% if e.opt %}y{% endif %}` | ok `` | ERR UndefinedError |
| if not | `{% if not e.opt %}y{% endif %}` | ok `y` | ERR UndefinedError |
| and | `{% if e.opt and e.req %}y{% endif %}` | ok `` | ERR UndefinedError |
| or fallback | `{{ e.opt or 'x' }}` | ok `x` | ERR UndefinedError |
| inline if | `{{ 'a' if e.opt else 'b' }}` | ok `b` | ERR UndefinedError |
| inline if without else (prints undefined) | `{{ 'a' if e.opt }}` | ok `` | ERR UndefinedError |
| is defined | `{% if e.opt is defined %}y{% endif %}` | ok `` | ok `` |
| default filter | `{{ e.opt\|default('d') }}` | ok `d` | ok `d` |
| == comparison | `{% if e.opt == 'x' %}y{% endif %}` | ERR UndefinedError | ERR UndefinedError |
| != between two absent | `{% if e.opt != e.opt2 %}y{% endif %}` | ERR UndefinedError | ERR UndefinedError |
| < comparison | `{% if e.opt < 3 %}y{% endif %}` | ERR UndefinedError | ERR UndefinedError |
| print | `{{ e.opt }}` | ERR UndefinedError | ERR UndefinedError |
| reach through | `{{ e.opt.deeper }}` | ERR UndefinedError | ERR UndefinedError |
| loop over | `{% for x in e.opt %}{% endfor %}` | ERR UndefinedError | ERR UndefinedError |
| filter on | `{{ e.opt\|lower }}` | ERR UndefinedError | ERR UndefinedError |
| concat | `{{ 'a' ~ e.opt }}` | ERR UndefinedError | ERR UndefinedError |
| set alias then is defined | `{% set v = e.opt %}{% if v is defined %}y{% endif %}` | ok `` | ok `` |
| with alias then if | `{% with v = e.opt %}{% if v %}y{% endif %}{% endwith %}` | ok `` | ERR UndefinedError |
| map literal value into attrs | `{{ {'k': e.opt}\|attrs }}` | ok `{'k': undefined}` | ok `{'k': undefined}` |
| pass absent to macro arg with default | `{% macro m(a, b='dflt') %}[{{ b }}]{% endmacro %}{{ m(1, b=e.opt) }}` | ok `[dflt]` | ok `[dflt]` |
| pass absent to optional macro arg, guarded | `{% macro m(a, b=none) %}[{% if b %}{{ b }}{% endif %}]{% endmacro %}{{ m(1, b=e.opt) }}` | ok `[]` | ok `[]` |
| omit a required macro arg | `{% macro m(a) %}[{{ a }}]{% endmacro %}{{ m() }}` | ERR UndefinedError | ERR UndefinedError |
| if caller without call block | `{% macro m(a) %}[{% if caller %}c{% endif %}]{% endmacro %}{{ m(1) }}` | ok `[]` | ERR UndefinedError |
| caller is defined | `{% macro m(a) %}[{% if caller is defined %}c{% endif %}]{% endmacro %}{{ m(1) }}` | ok `[]` | ok `[]` |
| print unknown variable | `{{ nosuch }}` | ERR UndefinedError | ERR UndefinedError |
| if on unknown top-level variable | `{% if nosuch %}y{% endif %}` | ok `` | ERR UndefinedError |
| unknown filter | `{{ e.req\|nosuchfilter }}` | ERR UnknownFilter | ERR UnknownFilter |
