//! Changes seeded into a copy of the Parity baseline: those the test must
//! catch, and those that decode to the same DOM, or that a Parity exception
//! covers, which it must let through. Each edit must still apply, so a new
//! baseline can't turn a seed into a no-op.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use parity::Difference;

pub(crate) struct Seed {
    pub(crate) name: &'static str,
    edits: &'static [Edit],
    pub(crate) expect: fn(&[Difference]) -> bool,
}

enum Edit {
    /// In one file, or in every Page for `*`. It must occur.
    Replace(&'static str, &'static str, &'static str),
    Delete(&'static str),
    Rename(&'static str, &'static str),
    Write(&'static str, &'static str),
}

const ALT: &str = "Two hornbills perched on a street lamp against a pale sky.";
const SHEETS: &str = "<link rel=\"stylesheet\" href=\"/_astro/about.DhiEpsgV.css\">";

fn dom(differences: &[Difference], on: &str) -> bool {
    matches!(differences, [Difference::Dom { page, .. }] if page == on)
}

pub(crate) const SEEDS: &[Seed] = &[
    // Caught.
    Seed {
        name: "a Page removed",
        edits: &[Edit::Delete("blog/first/index.html")],
        // With it go the links to it, and the one to its hero image, which
        // `hero-images` names by its stem.
        expect: |d| {
            d == [
                Difference::Missing("/blog/first/index.html".into()),
                Difference::MissingRef("hero:plane".into()),
                Difference::Broken("/blog/first/".into()),
            ]
        },
    },
    Seed {
        name: "a Page added",
        edits: &[Edit::Write("blog/second/index.html", "<!DOCTYPE html>")],
        expect: |d| matches!(d, [Difference::Added(p)] if p == "/blog/second/index.html"),
    },
    Seed {
        name: "a link retargeted",
        edits: &[Edit::Replace(
            "index.html",
            "href=\"/blog\"",
            "href=\"/archive\"",
        )],
        expect: |d| dom(d, "/index.html"),
    },
    Seed {
        name: "an image width deleted",
        edits: &[Edit::Delete("_astro/beach-market.ChlaUixY_1DHuWQ.jpg")],
        expect: |d| matches!(d, [Difference::Broken(t)] if t == "asset:beach-market.jpg"),
    },
    Seed {
        name: "a heading reworded",
        edits: &[Edit::Replace(
            "about/index.html",
            "<h1 class=\"panel-title\">About",
            "<h1 class=\"panel-title\">About me",
        )],
        expect: |d| dom(d, "/about/index.html"),
    },
    Seed {
        name: "an alt text changed",
        edits: &[Edit::Replace(
            "about/index.html",
            ALT,
            "Two hornbills on a lamp.",
        )],
        expect: |d| dom(d, "/about/index.html"),
    },
    Seed {
        name: "stylesheets reordered",
        // The Film's fallback moved after the bundles, which compare as one
        // run (`scoped-styles`).
        edits: &[
            Edit::Replace("about/index.html", SHEETS, ""),
            Edit::Replace(
                "about/index.html",
                "<style>",
                "<link rel=\"stylesheet\" href=\"/_astro/about.DhiEpsgV.css\"><style>",
            ),
        ],
        expect: |d| dom(d, "/about/index.html"),
    },
    Seed {
        name: "code text changed",
        edits: &[Edit::Replace(
            "blog/extending-c-stdlib/index.html",
            "> s_vec</span>",
            "> s_vector</span>",
        )],
        expect: |d| dom(d, "/blog/extending-c-stdlib/index.html"),
    },
    // Let through.
    Seed {
        name: "markup rewritten to the same DOM",
        edits: &[
            Edit::Replace("index.html", "<head>", "<head>\n  <!-- rewritten -->\n  "),
            Edit::Replace("index.html", "</a><a ", "</a>\n      <a "),
            Edit::Replace(
                "index.html",
                "class=\"nav-link\" data-nav-link",
                "data-nav-link=\"\" class=\"nav-link\"",
            ),
            Edit::Replace(
                "index.html",
                "data-section=\"about\" data-reveal",
                "data-section=about data-reveal",
            ),
            Edit::Replace("index.html", "i'm yiyuan", "i&#39;m yiyuan"),
        ],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "assets hashed differently",
        edits: &[
            Edit::Rename("_astro/about.DhiEpsgV.css", "_astro/about.Zz9_Yy8-.css"),
            Edit::Replace(
                "*",
                "/_astro/about.DhiEpsgV.css",
                "/_astro/about.Zz9_Yy8-.css",
            ),
        ],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "head reordered outside its stylesheets",
        edits: &[
            Edit::Replace(
                "index.html",
                "<meta name=\"author\" content=\"Yiyuan Li\">",
                "",
            ),
            Edit::Replace(
                "index.html",
                "</head>",
                "<meta name=\"author\" content=\"Yiyuan Li\"></head>",
            ),
        ],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "code highlighted differently (class-highlighting)",
        edits: &[Edit::Replace(
            "blog/extending-c-stdlib/index.html",
            "<span class=\"line\">",
            "<span class=\"y-line\">",
        )],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "apex origin and trailing slashes (apex-origin, trailing-slash-hrefs)",
        edits: &[
            Edit::Replace("*", "https://www.eyuan.me/", "https://eyuan.me/"),
            Edit::Replace("*", "href=\"/about\"", "href=\"/about/\""),
        ],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "the Engine's asset names (assets-dir)",
        edits: &[
            Edit::Rename(
                "_astro/about.DhiEpsgV.css",
                "assets/about-0123456789abcdef.css",
            ),
            Edit::Replace(
                "*",
                "/_astro/about.DhiEpsgV.css",
                "/assets/about-0123456789abcdef.css",
            ),
        ],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "robots.txt added (robots-txt)",
        edits: &[Edit::Write("robots.txt", "User-agent: *\n")],
        expect: <[Difference]>::is_empty,
    },
    Seed {
        name: "unreferenced fonts dropped (unreferenced-files)",
        edits: &[
            Edit::Delete("fonts/atkinson-bold.woff"),
            Edit::Delete("fonts/atkinson-regular.woff"),
        ],
        expect: <[Difference]>::is_empty,
    },
];

/// Why a seed couldn't be applied, or the baseline copied.
#[derive(Debug)]
pub(crate) enum Error {
    /// A `Replace` whose text isn't in its file.
    Missing {
        from: &'static str,
        file: &'static str,
    },
    /// About the file or folder at the path.
    At(PathBuf, io::Error),
    Io(io::Error),
    Walk(walkdir::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Missing { from, file } => write!(f, "{from:?} isn't in {file}"),
            Error::At(path, e) => write!(f, "{}: {e}", path.display()),
            Error::Io(e) => e.fmt(f),
            Error::Walk(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

impl Seed {
    pub(crate) fn apply(&self, site: &Path) -> Result<(), Error> {
        for edit in self.edits {
            match edit {
                Edit::Replace(file, from, to) => {
                    let files = match *file {
                        "*" => pages(site),
                        one => vec![site.join(one)],
                    };
                    let mut found = false;
                    for file in files {
                        let text = read(&file)?;
                        found |= text.contains(from);
                        fs::write(&file, text.replace(from, to)).map_err(Error::Io)?;
                    }
                    if !found {
                        return Err(Error::Missing { from, file });
                    }
                }
                Edit::Delete(file) => {
                    fs::remove_file(site.join(file)).map_err(|e| Error::At(file.into(), e))?
                }
                Edit::Rename(from, to) => {
                    let to = site.join(to);
                    fs::create_dir_all(to.parent().expect("a seed's file is in the Site"))
                        .map_err(Error::Io)?;
                    fs::rename(site.join(from), to).map_err(|e| Error::At(from.into(), e))?
                }
                Edit::Write(file, text) => {
                    let file = site.join(file);
                    fs::create_dir_all(file.parent().expect("a seed's file is in the Site"))
                        .map_err(Error::Io)?;
                    fs::write(file, text).map_err(Error::Io)?
                }
            }
        }
        Ok(())
    }
}

fn read(file: &Path) -> Result<String, Error> {
    fs::read_to_string(file).map_err(|e| Error::At(file.to_path_buf(), e))
}

fn pages(site: &Path) -> Vec<std::path::PathBuf> {
    walkdir::WalkDir::new(site)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "html"))
        .map(|e| e.into_path())
        .collect()
}

pub(crate) fn copy(from: &Path, to: &Path) -> Result<(), Error> {
    for entry in walkdir::WalkDir::new(from) {
        let entry = entry.map_err(Error::Walk)?;
        let target = to.join(
            entry
                .path()
                .strip_prefix(from)
                .expect("WalkDir yields only paths under the folder it walks"),
        );
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target).map_err(Error::Io)?;
        } else {
            fs::copy(entry.path(), &target).map_err(Error::Io)?;
        }
    }
    Ok(())
}
