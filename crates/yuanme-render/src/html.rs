//! A Document's body as HTML (#47). The shape is what the Reference site's
//! remark pipeline emits (mdast-util-to-hast, then hast-util-to-html), so a
//! Post renders byte for byte as it does on Astro; Org emits the same HTML
//! wherever it has the same construct (#16).

use std::collections::BTreeMap;

use yuanme_content::document::{Align, Document, Kind, Links, NodeId, footnote_identifier};

/// `links` says where resolved links go; any other link keeps the URL it
/// was written with.
pub fn body(doc: &Document, links: &Links) -> String {
    let mut w = Writer {
        doc,
        links,
        out: String::new(),
        references: BTreeMap::new(),
    };
    let root = &doc.node(Document::ROOT).children;
    let blocks: Vec<NodeId> = root
        .iter()
        .copied()
        .filter(|&c| !matches!(doc.node(c).kind, Kind::FootnoteDefinition { .. }))
        .collect();
    w.join(&blocks);
    w.footer(root);
    w.out
}

struct Writer<'d> {
    doc: &'d Document,
    links: &'d Links,
    out: String,
    /// How often each footnote has been referenced so far.
    references: BTreeMap<String, usize>,
}

impl Writer<'_> {
    /// Siblings separated by a newline.
    fn join(&mut self, ids: &[NodeId]) {
        for (i, &id) in ids.iter().enumerate() {
            if i > 0 {
                self.out.push('\n');
            }
            self.node(id);
        }
    }

    /// Block children inside a container: each on its own line.
    fn wrap(&mut self, ids: &[NodeId]) {
        self.out.push('\n');
        self.join(ids);
        if !ids.is_empty() {
            self.out.push('\n');
        }
    }

    fn inline(&mut self, id: NodeId) {
        for &c in &self.doc.node(id).children {
            self.node(c);
        }
    }

    fn element(&mut self, tag: &str, id: NodeId) {
        self.out.push_str(&format!("<{tag}>"));
        self.inline(id);
        self.out.push_str(&format!("</{tag}>"));
    }

    fn block(&mut self, open: &str, tag: &str, id: NodeId) {
        self.out.push_str(open);
        let children = self.doc.node(id).children.clone();
        self.wrap(&children);
        self.out.push_str(&format!("</{tag}>"));
    }

    fn node(&mut self, id: NodeId) {
        let doc = self.doc;
        let node = doc.node(id);
        match &node.kind {
            Kind::Root => unreachable!("the root is not a child"),
            Kind::Paragraph => self.element("p", id),
            Kind::Heading {
                level,
                id: anchor,
                attrs,
            } => {
                self.out
                    .push_str(&format!("<h{level} id=\"{}\"", attribute(anchor)));
                for (name, value) in attrs {
                    self.out
                        .push_str(&format!(" {name}=\"{}\"", attribute(value)));
                }
                self.out.push('>');
                self.inline(id);
                self.out.push_str(&format!("</h{level}>"));
            }
            Kind::ThematicBreak => self.out.push_str("<hr>"),
            Kind::BlockQuote => self.block("<blockquote>", "blockquote", id),
            Kind::List {
                ordered,
                start,
                tight,
            } => self.list(id, *ordered, *start, !tight),
            Kind::Item { .. } => unreachable!("written by its list"),
            Kind::CodeBlock { lang, text: code } => {
                self.out.push_str("<pre><code");
                if let Some(lang) = lang {
                    self.out
                        .push_str(&format!(" class=\"language-{}\"", attribute(lang)));
                }
                self.out.push('>');
                if !code.is_empty() {
                    self.out.push_str(&text(code));
                    self.out.push('\n');
                }
                self.out.push_str("</code></pre>");
            }
            Kind::HtmlBlock(raw) | Kind::HtmlInline(raw) => {
                self.out.push_str(&raw.replace('\0', "\u{FFFD}"))
            }
            Kind::Table { align } => self.table(id, align),
            Kind::TableRow { .. } | Kind::TableCell => unreachable!("written by its table"),
            Kind::FootnoteDefinition { .. } => unreachable!("written in the footer"),
            Kind::Directive { name } => {
                let open = format!("<div class=\"{}\">", attribute(name));
                self.block(&open, "div", id);
            }
            Kind::Text(s) => self.out.push_str(&text(s)),
            Kind::Code(s) => self.out.push_str(&format!("<code>{}</code>", text(s))),
            Kind::Emphasis => self.element("em", id),
            Kind::Strong => self.element("strong", id),
            Kind::Strikethrough => self.element("del", id),
            Kind::Link { url, title } => {
                let href = normalize_uri(self.links.get(&id).unwrap_or(url));
                self.out
                    .push_str(&format!("<a href=\"{}\"", attribute(&href)));
                if let Some(title) = title {
                    self.out
                        .push_str(&format!(" title=\"{}\"", attribute(title)));
                }
                self.out.push('>');
                self.inline(id);
                self.out.push_str("</a>");
            }
            Kind::Image { url, title } => {
                let alt = alt(doc, id);
                self.out.push_str(&format!(
                    "<img src=\"{}\" alt=\"{}\"",
                    attribute(&normalize_uri(url)),
                    attribute(&alt)
                ));
                if let Some(title) = title {
                    self.out
                        .push_str(&format!(" title=\"{}\"", attribute(title)));
                }
                self.out.push('>');
            }
            Kind::LineBreak => self.out.push_str("<br>\n"),
            Kind::FootnoteReference { label, number } => self.reference(label, *number),
            // Until math renders (#50), its TeX shows as code.
            Kind::Math {
                display: false,
                tex,
            } => {
                self.out.push_str(&format!(
                    "<code class=\"language-math math-inline\">{}</code>",
                    text(tex)
                ));
            }
            Kind::Math { display: true, tex } => {
                self.out.push_str(&format!(
                    "<code class=\"language-math math-display\">{}</code>",
                    text(tex)
                ));
            }
        }
    }

    /// mdast-util-to-hast's list and listItem: a tight item's paragraphs
    /// lose their `<p>`, and a task item starts with a checkbox.
    fn list(&mut self, id: NodeId, ordered: bool, start: u64, loose: bool) {
        let doc = self.doc;
        let items = &doc.node(id).children;
        let tasks = items
            .iter()
            .any(|&i| matches!(doc.node(i).kind, Kind::Item { checked: Some(_) }));
        let tag = if ordered { "ol" } else { "ul" };
        self.out.push('<');
        self.out.push_str(tag);
        if ordered && start != 1 {
            self.out.push_str(&format!(" start=\"{start}\""));
        }
        if tasks {
            self.out.push_str(" class=\"contains-task-list\"");
        }
        self.out.push_str(">\n");
        for (n, &item) in items.iter().enumerate() {
            if n > 0 {
                self.out.push('\n');
            }
            self.item(item, loose);
        }
        if !items.is_empty() {
            self.out.push('\n');
        }
        self.out.push_str(&format!("</{tag}>"));
    }

    fn item(&mut self, id: NodeId, loose: bool) {
        let doc = self.doc;
        let node = doc.node(id);
        let Kind::Item { checked } = node.kind else {
            unreachable!("a list holds items")
        };
        let is_p = |c: NodeId| matches!(doc.node(c).kind, Kind::Paragraph);
        let children = &node.children;
        match checked {
            Some(_) => self.out.push_str("<li class=\"task-list-item\">"),
            None => self.out.push_str("<li>"),
        }
        // The checkbox goes at the start of the first paragraph, which is
        // made if the item doesn't start with one.
        let checkbox = checked.map(|on| {
            if on {
                "<input type=\"checkbox\" checked disabled>"
            } else {
                "<input type=\"checkbox\" disabled>"
            }
        });
        let head_is_p = children.first().is_some_and(|&c| is_p(c));
        let mut parts: Vec<Option<NodeId>> = children.iter().copied().map(Some).collect();
        if checkbox.is_some() && !head_is_p {
            parts.insert(0, None);
        }
        let count = parts.len();
        for (i, part) in parts.iter().enumerate() {
            let p = part.is_none_or(is_p);
            if loose || i != 0 || !p {
                self.out.push('\n');
            }
            if loose && p {
                self.out.push_str("<p>");
            }
            if i == 0
                && let Some(checkbox) = checkbox
            {
                self.out.push_str(checkbox);
                if part.is_some_and(|c| !doc.node(c).children.is_empty()) {
                    self.out.push(' ');
                }
            }
            match part {
                Some(c) if p => self.inline(*c),
                Some(c) => self.node(*c),
                None => {}
            }
            if loose && p {
                self.out.push_str("</p>");
            }
            if i + 1 == count && (loose || !p) {
                self.out.push('\n');
            }
        }
        self.out.push_str("</li>");
    }

    /// mdast-util-to-hast's table: the first row is the head, every row has
    /// a cell per column, and the body is left out when it has no rows.
    /// rehype-raw then reparses it, which moves every newline between the
    /// table's own tags to before the table.
    fn table(&mut self, id: NodeId, align: &[Align]) {
        let rows = self.doc.node(id).children.clone();
        let start = self.out.len();
        let mut newlines = 2;
        self.out.push_str("<table>");
        let (head, body) = rows
            .split_first()
            .map_or((None, &[][..]), |(h, b)| (Some(*h), b));
        if let Some(head) = head {
            self.out.push_str("<thead>");
            newlines += 2 + self.row(head, "th", align);
            self.out.push_str("</thead>");
        }
        if !body.is_empty() {
            self.out.push_str("<tbody>");
            newlines += 2 + body.len() - 1;
            for &row in body {
                newlines += self.row(row, "td", align);
            }
            self.out.push_str("</tbody>");
            newlines += 1;
        }
        self.out.push_str("</table>");
        self.out.insert_str(start, &"\n".repeat(newlines));
    }

    /// Writes a row and says how many newlines mdast-util-to-hast puts
    /// between its tags.
    fn row(&mut self, id: NodeId, tag: &str, align: &[Align]) -> usize {
        let cells = self.doc.node(id).children.clone();
        self.out.push_str("<tr>");
        for (i, a) in align.iter().enumerate() {
            let align = match a {
                Align::None => "",
                Align::Left => " align=\"left\"",
                Align::Center => " align=\"center\"",
                Align::Right => " align=\"right\"",
            };
            self.out.push_str(&format!("<{tag}{align}>"));
            if let Some(&cell) = cells.get(i) {
                self.inline(cell);
            }
            self.out.push_str(&format!("</{tag}>"));
        }
        self.out.push_str("</tr>");
        align.len() + 1
    }

    fn reference(&mut self, label: &str, number: usize) {
        let key = footnote_identifier(label);
        let count = self.references.entry(key.clone()).or_default();
        *count += 1;
        let safe = normalize_uri(&key.to_lowercase());
        let again = if *count > 1 {
            format!("-{count}")
        } else {
            String::new()
        };
        self.out.push_str(&format!(
            "<sup><a href=\"#user-content-fn-{safe}\" id=\"user-content-fnref-{safe}{again}\" data-footnote-ref=\"\" aria-describedby=\"footnote-label\">{number}</a></sup>"
        ));
    }

    /// The links back to footnote `n`'s references so far, joined by
    /// `between`.
    fn backrefs(&self, key: &str, n: usize, between: &str) -> String {
        let safe = normalize_uri(&key.to_lowercase());
        let backrefs: Vec<String> = (1..=self.references[key])
            .map(|r| {
                let again = if r > 1 { format!("-{r}") } else { String::new() };
                let sup = if r > 1 { format!("<sup>{r}</sup>") } else { String::new() };
                format!(
                    "<a href=\"#user-content-fnref-{safe}{again}\" data-footnote-backref=\"\" aria-label=\"Back to reference {n}{again}\" class=\"data-footnote-backref\">↩{sup}</a>"
                )
            })
            .collect();
        backrefs.join(between)
    }

    /// mdast-util-to-hast's footer: the referenced definitions, in the
    /// order of their numbers, each with links back to the references
    /// written before its end.
    fn footer(&mut self, root: &[NodeId]) {
        let doc = self.doc;
        let definitions: BTreeMap<String, NodeId> = root
            .iter()
            .filter_map(|&id| match &doc.node(id).kind {
                Kind::FootnoteDefinition { label } => Some((footnote_identifier(label), id)),
                _ => None,
            })
            .collect();
        let numbered: BTreeMap<usize, String> = doc
            .descendants(Document::ROOT)
            .into_iter()
            .filter_map(|id| match &doc.node(id).kind {
                Kind::FootnoteReference { label, number } if *number > 0 => {
                    Some((*number, footnote_identifier(label)))
                }
                _ => None,
            })
            .collect();
        let mut items = Vec::new();
        for (&n, key) in &numbered {
            let Some(&def) = definitions.get(key) else {
                continue;
            };
            let mut item = std::mem::take(&mut self.out);
            let safe = normalize_uri(&key.to_lowercase());
            let children = doc.node(def).children.clone();
            self.out
                .push_str(&format!("<li id=\"user-content-fn-{safe}\">\n"));
            let last_is_p = children
                .last()
                .is_some_and(|&c| matches!(doc.node(c).kind, Kind::Paragraph));
            for (i, &c) in children.iter().enumerate() {
                if i > 0 {
                    self.out.push('\n');
                }
                if i + 1 == children.len() && last_is_p {
                    self.out.push_str("<p>");
                    self.inline(c);
                    self.out.push(' ');
                    let backrefs = self.backrefs(key, n, " ");
                    self.out.push_str(&backrefs);
                    self.out.push_str("</p>");
                } else {
                    self.node(c);
                }
            }
            if !last_is_p {
                if !children.is_empty() {
                    self.out.push('\n');
                }
                // Each link back is a block of its own, as is the space.
                let backrefs = self.backrefs(key, n, "\n \n");
                self.out.push_str(&backrefs);
            }
            self.out.push_str("\n</li>");
            std::mem::swap(&mut self.out, &mut item);
            items.push(item);
        }
        if items.is_empty() {
            return;
        }
        self.out.push_str("\n<section data-footnotes=\"\" class=\"footnotes\"><h2 class=\"sr-only\" id=\"footnote-label\">Footnotes</h2>\n<ol>\n");
        self.out.push_str(&items.join("\n"));
        self.out.push_str("\n</ol>\n</section>");
    }
}

/// An image's alt text: its children as plain text, raw HTML included.
fn alt(doc: &Document, id: NodeId) -> String {
    let mut out = String::new();
    for c in doc.descendants(id) {
        match &doc.node(c).kind {
            Kind::Text(s) | Kind::Code(s) | Kind::HtmlInline(s) => out.push_str(s),
            Kind::Math { tex, .. } => out.push_str(tex),
            _ => {}
        }
    }
    out
}

/// hast-util-to-html's text: `<` and `&` as hexadecimal references. A NUL,
/// which micromark replaces, is U+FFFD here and in attributes.
fn text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&#x3C;"),
            '&' => out.push_str("&#x26;"),
            '\0' => out.push('\u{FFFD}'),
            c => out.push(c),
        }
    }
    out
}

/// hast-util-to-html's double-quoted attribute value, safe for HTML and
/// XML alike: `"`, `&`, `'` and `` ` `` as hexadecimal references.
fn attribute(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\0' => out.push('\u{FFFD}'),
            '"' | '&' | '\'' | '`' => out.push_str(&format!("&#x{:X};", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// micromark's `normalizeUri`: percent-encodes what a URL can't hold and
/// keeps escapes already written.
fn normalize_uri(url: &str) -> String {
    let bytes = url.as_bytes();
    let mut out = String::with_capacity(url.len());
    let mut i = 0;
    for c in url.chars() {
        let at = i;
        i += c.len_utf8();
        let kept = match c {
            '%' => {
                bytes.get(at + 1).is_some_and(u8::is_ascii_alphanumeric)
                    && bytes.get(at + 2).is_some_and(u8::is_ascii_alphanumeric)
            }
            // `[!#$&-;=?-Z_a-z~]`
            '!' | '#' | '$' | '=' | '_' | '~' | '&'..=';' | '?'..='Z' | 'a'..='z' => true,
            _ => false,
        };
        if kept {
            out.push(c);
        } else {
            for b in c.to_string().bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}
