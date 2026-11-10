//! What each Page and stylesheet names, and where it is among the stage's
//! assets, for its placeholder.

#[derive(Clone)]
pub(super) enum Use {
    Bundle(String),
    /// A Script, by the project file it starts from.
    Script(String),
    Copy(String),
    /// Written as is.
    Url(String),
}

/// Where each asset the Pages name is among the stage's assets: the copies,
/// then the stylesheet bundles, then the Script chunks. The link pass needs
/// an asset after those it names, and bundles name copies, while chunks
/// name only each other.
pub(super) struct Order<'s> {
    pub(super) copies: Vec<String>,
    pub(super) stylesheets: &'s [String],
    pub(super) scripts: &'s [String],
    /// The chunk of each entry: each Script's, then each Page Script's,
    /// then the dev client's.
    pub(super) entries: &'s [usize],
    /// The chunks each chunk imports, as [`Scripts::preloads`](super::scripts::Scripts::preloads) has them.
    pub(super) preloads: &'s [Vec<usize>],
    pub(super) chunks_at: usize,
}

impl Order<'_> {
    pub(super) fn index(&self, use_: &Use) -> usize {
        match use_ {
            Use::Copy(path) => self
                .copies
                .binary_search(path)
                .expect("references and bundles add each Copy they make to `copies`"),
            Use::Bundle(path) => {
                let at = self.stylesheets.iter().position(|e| e == path);
                self.copies.len() + at.expect("references adds each Bundle to `stylesheets`")
            }
            Use::Script(path) => self.chunk(self.script(path)),
            Use::Url(_) => unreachable!("a URL is no asset"),
        }
    }

    /// The asset of chunk `chunk`.
    pub(super) fn chunk(&self, chunk: usize) -> usize {
        self.chunks_at + chunk
    }

    /// The chunk of the Script that starts from `path`.
    pub(super) fn script(&self, path: &str) -> usize {
        let entry = self
            .scripts
            .iter()
            .position(|e| e == path)
            .expect("references adds each Script to `scripts`");
        self.entries[entry]
    }

    /// The chunk of Page Script `i`.
    pub(super) fn page_script(&self, i: usize) -> usize {
        self.entries[self.scripts.len() + i]
    }

    /// The asset of the Site-wide Sidecar stylesheet, the last bundle.
    pub(super) fn sidecars(&self) -> usize {
        self.copies.len() + self.stylesheets.len()
    }
}
