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
