//! Tables, with their alignment cookies and head rows.

use yuanme_org::types::Table;

use super::Lower;
use crate::document::{Align, Kind, NodeId};

impl<'s> Lower<'s> {
    pub(super) fn table(&mut self, t: &Table<'s>, parent: NodeId) {
        let at = self.at(t.source);
        // A row of alignment cookies such as `<r>` sets the columns'
        // alignment and isn't shown, as `:with-special-rows` is nil.
        let cookie = |cell: &str| {
            let c = cell.trim();
            c.len() >= 2
                && c.starts_with('<')
                && c.ends_with('>')
                && c[1..c.len() - 1]
                    .trim_start_matches(['l', 'r', 'c'])
                    .chars()
                    .all(|d| d.is_ascii_digit())
        };
        let special = |row: &yuanme_org::types::TableRow<'_>| {
            !row.children.is_empty()
                && row.children.iter().any(|c| cookie(c.contents))
                && row
                    .children
                    .iter()
                    .all(|c| c.contents.trim().is_empty() || cookie(c.contents))
        };
        let columns = t
            .children
            .iter()
            .map(|r| r.children.len())
            .max()
            .unwrap_or(0);
        let mut align = vec![Align::None; columns];
        for row in t.children.iter().filter(|r| special(r)) {
            for (i, c) in row.children.iter().enumerate() {
                let c = c.contents.trim();
                if cookie(c) {
                    align[i] = match c.as_bytes()[1] {
                        b'l' => Align::Left,
                        b'r' => Align::Right,
                        b'c' => Align::Center,
                        _ => align[i],
                    };
                }
            }
        }
        let rows: Vec<_> = t.children.iter().filter(|r| !special(r)).collect();
        // Rows before the first rule are the head, if any row follows it.
        let rule = rows.iter().position(|r| r.children.is_empty());
        let heads = match rule {
            Some(r) if rows[r..].iter().any(|r| !r.children.is_empty()) => r,
            _ => 0,
        };
        let id = self.doc.push(parent, Kind::Table { align }, at);
        let a = &t.affiliated_keywords;
        self.name(a, id);
        let attrs = self.attr_html(a);
        self.doc.attrs_mut(id).extend(attrs);
        self.caption(a, id);
        for (n, row) in rows.iter().enumerate() {
            if row.children.is_empty() {
                continue;
            }
            let at = self.at(row.source);
            let r = self.doc.push(id, Kind::TableRow { head: n < heads }, at);
            for cell in &row.children {
                let at = self.at(cell.contents);
                let c = self.doc.push(r, Kind::TableCell, at);
                for o in &cell.children {
                    self.object(o, c);
                }
                self.trim(c);
            }
        }
    }
}
