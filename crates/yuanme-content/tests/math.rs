//! Math converted at parse time (#50): amsmath's numbering per Entry in
//! both Source formats, macros, and the Labels math shares with headings.

use yuanme_content::document::{Document, Kind};
use yuanme_content::{markdown, org};
use yuanme_diag::{Diagnostic, Files};

fn markdown(text: &str) -> (Document, Vec<Diagnostic>) {
    let mut files = Files::default();
    let file = files.add("a.md", text);
    let (doc, diagnostics) = markdown::document(&files, file, 0..text.len());
    (doc.expect("it parses"), diagnostics)
}

fn org(text: &str) -> (Document, Vec<Diagnostic>) {
    let mut files = Files::default();
    let file = files.add("a.org", text);
    let (doc, diagnostics) = org::document(&files, file);
    (doc.expect("it parses"), diagnostics)
}

/// Each equation number math-core wrote, with the Label it is the id of,
/// and each reference's text, in document order: `eq:a=(1)`, `(2)`,
/// `#eq:a→(1)`, and an Org `\ref` as `ref eq:a→1`.
fn numbers(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    let mut ids: Vec<_> = doc.descendants(Document::ROOT);
    ids.sort_by_key(|&id| doc.node(id).span.start);
    for id in ids {
        match &doc.node(id).kind {
            Kind::Math { mathml, .. } => {
                let mut rest = mathml.as_str();
                while let Some(i) = rest.find("<mtd") {
                    rest = &rest[i..];
                    let tag_end = rest.find('>').unwrap();
                    let tag = &rest[..tag_end];
                    let cell = &rest[tag_end + 1..rest.find("</mtd>").unwrap()];
                    if let Some(n) = cell
                        .strip_prefix("<mtext>")
                        .and_then(|c| c.strip_suffix("</mtext>"))
                    {
                        match tag.split_once(" id=\"") {
                            Some((_, id)) => {
                                out.push(format!("{}={n}", &id[..id.find('"').unwrap()]))
                            }
                            None => out.push(n.to_string()),
                        }
                    }
                    rest = &rest[tag_end..];
                }
                let mut rest = mathml.as_str();
                while let Some(i) = rest.find("<a href=\"") {
                    rest = &rest[i + 9..];
                    let href = &rest[..rest.find('"').unwrap()];
                    let text = &rest[rest.find('>').unwrap() + 1..rest.find("</a>").unwrap()];
                    out.push(format!("{href}→{text}"));
                }
            }
            Kind::Ref { label, .. } => {
                out.push(format!("ref {label}→{}", doc.node(id).kind.ref_text()))
            }
            _ => {}
        }
    }
    out
}

/// The same equations, as each Source format writes them.
const MARKDOWN: &str = r"By $\eqref{eq:wave}$, which comes later, and:

$$
\begin{equation} E = mc^2 \label{eq:energy} \end{equation}
$$

$$ x + y $$

$$
\begin{align}
a &= b \label{eq:ab} \\
c &= d \notag \\
e &= f
\end{align}
$$

$$\begin{equation*} g \end{equation*}$$

```math
\begin{gather}
h \\
i \tag{*} \label{eq:star}
\end{gather}
```

$$\begin{multline} j \\ k \label{eq:wave} \end{multline}$$

So $\eqref{eq:energy}$ and $\eqref{eq:star}$.
";

const ORG: &str = r"By \eqref{eq:wave}, which comes later, and:

\begin{equation}
E = mc^2 \label{eq:energy}
\end{equation}

\[ x + y \]

\begin{align}
a &= b \label{eq:ab} \\
c &= d \notag \\
e &= f
\end{align}

\begin{equation*}
g
\end{equation*}

\begin{gather}
h \\
i \tag{*} \label{eq:star}
\end{gather}

$$\begin{multline} j \\ k \label{eq:wave} \end{multline}$$

So $\eqref{eq:energy}$, \ref{eq:energy} and \eqref{eq:star}.
";

#[test]
fn equations_are_numbered_as_amsmath_numbers_them_in_both_formats() {
    let (md, diagnostics) = markdown(MARKDOWN);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        numbers(&md),
        [
            "#eq:wave→(5)",
            "eq:energy=(1)",
            "eq:ab=(2)",
            "(3)",
            "(4)",
            "eq:star=(*)",
            "eq:wave=(5)",
            "#eq:energy→(1)",
            "#eq:star→(*)",
        ]
    );
    let (org, diagnostics) = org(ORG);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        numbers(&org),
        [
            "ref eq:wave→(5)",
            "eq:energy=(1)",
            "eq:ab=(2)",
            "(3)",
            "(4)",
            "eq:star=(*)",
            "eq:wave=(5)",
            "#eq:energy→(1)",
            "ref eq:energy→1",
            "ref eq:star→(*)",
        ]
    );
}

#[test]
fn a_newcommand_holds_for_the_rest_of_the_entry() {
    let (doc, diagnostics) =
        markdown("$\\newcommand{\\RR}{\\mathbb{R}}$ and later $x \\in \\RR$.\n");
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert!(doc.math_chars().contains(&'ℝ'));
}

#[test]
fn latex_header_macros_define_the_entrys_math() {
    let text = r"#+TITLE: A
#+LATEX_HEADER: \usepackage{amsmath,amssymb}
#+LATEX_HEADER: \newcommand{\RR}{\mathbb{R}}
#+LATEX_HEADER: \DeclareMathOperator*{\Prob}{Pr}
#+LATEX_HEADER: \usepackage{tikz}

\(\Prob_{x \in \RR} f\)
";
    let (doc, diagnostics) = org(text);
    let got: Vec<_> = diagnostics
        .iter()
        .map(|d| (d.code.name, &text[d.primary().range.clone()]))
        .collect();
    assert_eq!(got, [("org/no-effect", r"\usepackage{tikz}")]);
    let chars = doc.math_chars();
    assert!(chars.contains(&'ℝ') && chars.contains(&'P'), "{chars:?}");
    let mathml = doc
        .descendants(Document::ROOT)
        .into_iter()
        .find_map(|id| match &doc.node(id).kind {
            Kind::Math { mathml, .. } => Some(mathml.clone()),
            _ => None,
        })
        .unwrap();
    assert!(mathml.contains("<mi>Pr</mi>"), "{mathml}");
}

#[test]
fn a_header_macro_with_an_error_is_reported_on_its_line() {
    let text = "#+LATEX_HEADER: \\newcommand{\\RR}{\\mathbb{R}\n\n\\(x\\)\n";
    let (_, diagnostics) = org(text);
    let got: Vec<_> = diagnostics
        .iter()
        .map(|d| (d.code.name, d.primary().range.start))
        .collect();
    assert_eq!(got.len(), 1, "{diagnostics:#?}");
    assert_eq!(got[0].0, "math/unsupported");
    assert!(got[0].1 >= "#+LATEX_HEADER: ".len() && got[0].1 <= text.find("\n\n").unwrap());
}

#[test]
fn a_generated_heading_id_never_takes_a_label() {
    let (doc, diagnostics) = markdown(
        "## Energy\n\n$$\\begin{equation} E \\label{energy} \\end{equation}$$\n\n## Energy\n",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let ids: Vec<_> = doc.headings().into_iter().map(|h| h.id).collect();
    assert_eq!(ids, ["energy-1", "energy-2"]);
}

#[test]
fn the_characters_math_shows_are_recorded() {
    let (doc, _) = markdown("$a < \\alpha$ and $$\\begin{equation} b \\end{equation}$$\n");
    let chars: String = doc.math_chars().into_iter().collect();
    // The equation number is shown in the math font too.
    assert_eq!(chars, "()1<abα");
}

/// Each Theorem block as `kind: name (title) #label`, and each other
/// directive as `div NAME`, in document order.
fn blocks(doc: &Document) -> Vec<String> {
    let mut out = Vec::new();
    for id in doc.descendants(Document::ROOT) {
        let node = doc.node(id);
        match &node.kind {
            Kind::Theorem { kind, name } => {
                let mut s = format!("{kind}: {name}");
                if let Some(&first) = node.children.first()
                    && doc.node(first).kind == Kind::Caption
                {
                    s.push_str(&format!(" ({})", doc.text(first)));
                }
                for (attr, value) in &node.attrs {
                    if attr == "id" {
                        s.push_str(&format!(" #{value}"));
                    }
                }
                out.push(s);
            }
            Kind::Directive { name } => out.push(format!("div {name}")),
            _ => {}
        }
    }
    out
}

/// The same Theorem blocks, as each Source format writes them.
const THEOREMS_MARKDOWN: &str = r#":::theorem[Fermat's little theorem]{#thm:fermat}
If $p$ is prime, then $a^p \equiv a \pmod p$.
:::

::::proof
By induction on $a$.

:::lemma[$p$ divides $\binom{p}{k}$]{#lem:binom .key}
For $0 < k < p$.
:::
::::

:::definition{#def:group}
A group.
:::

:::remark
Unnumbered.
:::

:::claim
Not a kind.
:::

:::corollary
Last.
:::
"#;

const THEOREMS_ORG: &str = r"#+NAME: thm:fermat
#+begin_theorem Fermat's little theorem
If \(p\) is prime, then \(a^p \equiv a \pmod p\).
#+end_theorem

#+begin_proof
By induction on \(a\).

#+NAME: lem:binom
#+ATTR_HTML: :class key
#+BEGIN_LEMMA \(p\) divides \(\binom{p}{k}\)
For \(0 < k < p\).
#+END_LEMMA
#+end_proof

#+NAME: def:group
#+begin_definition
A group.
#+end_definition

#+begin_remark
Unnumbered.
#+end_remark

#+begin_claim
Not a kind.
#+end_claim

#+begin_corollary
Last.
#+end_corollary
";

#[test]
fn theorem_blocks_are_numbered_on_one_counter_in_both_formats() {
    let want = [
        "theorem: Theorem 1 (Fermat’s little theorem) #thm:fermat",
        "proof: Proof",
        r"lemma: Lemma 2 (p divides \binom{p}{k}) #lem:binom",
        "definition: Definition 3 #def:group",
        "remark: Remark",
        "div claim",
        "corollary: Corollary 4",
    ];
    for (doc, diagnostics) in [markdown(THEOREMS_MARKDOWN), org(THEOREMS_ORG)] {
        assert!(diagnostics.is_empty(), "{diagnostics:#?}");
        assert_eq!(blocks(&doc), want);
        // A title's math is converted like the body's.
        assert!(doc.math_chars().contains(&'k'));
    }
}

/// Each link's URL and text, in document order.
fn links(doc: &Document) -> Vec<(String, String)> {
    let mut ids = doc.descendants(Document::ROOT);
    ids.sort_by_key(|&id| doc.node(id).span.start);
    ids.into_iter()
        .filter_map(|id| match &doc.node(id).kind {
            Kind::Link { url, .. } => Some((url.clone(), doc.text(id))),
            _ => None,
        })
        .collect()
}

#[test]
fn an_empty_link_to_a_label_shows_its_number() {
    let (md, diagnostics) = markdown(
        r"By [](#thm:a), [](#eq:e), [the theorem](#thm:a), [](#rem:a) and [](#sec).

## Sec {#sec}

:::lemma{#lem:z}
x
:::

:::theorem{#thm:a}
x
:::

:::remark{#rem:a}
y
:::

$$\begin{equation} E \label{eq:z} \end{equation}$$

$$\begin{equation} E \label{eq:e} \end{equation}$$
",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let got: Vec<_> = links(&md).into_iter().map(|(_, text)| text).collect();
    // A heading's link is left as it is written.
    assert_eq!(got, ["Theorem 2", "(2)", "the theorem", "Remark", ""]);

    let (org, diagnostics) = org(
        r"By [[thm:a]], [[#eq:e]], [[thm:a][the theorem]], [[#rem:a]] and [[Sec]].

* Sec

#+NAME: lem:z
#+begin_lemma
x
#+end_lemma

#+NAME: thm:a
#+begin_theorem
x
#+end_theorem

#+NAME: rem:a
#+begin_remark
y
#+end_remark

\begin{equation}
E \label{eq:z}
\end{equation}

\begin{equation}
E \label{eq:e}
\end{equation}
",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    assert_eq!(
        links(&org),
        [
            ("#thm:a".into(), "Theorem 2".into()),
            ("#eq:e".into(), "(2)".into()),
            ("#thm:a".into(), "the theorem".into()),
            ("#rem:a".into(), "Remark".into()),
            // Org shows a headline's title.
            ("#sec".into(), "Sec".into()),
        ]
    );
}
