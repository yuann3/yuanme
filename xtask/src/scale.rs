//! The Scale Site (#15, #73): 1,000 Posts of 8–16 KB, 70% Markdown and
//! 30% Org, an average of 5 code blocks a Post, math in 20% of them and 100
//! images, with tag, feed and sitemap pages, on the Reference site's
//! Templates. It is generated, not committed: the corpus (#14) is under
//! many licences, and what is made from it stays out of the repository.
//!
//! The prose, code and math come from the corpus, so the Engine parses,
//! highlights and converts the kind of text Authors write. The same seed
//! always makes the same Site.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const POSTS: usize = 1000;

/// The Reference site's Templates, Sidecars, Scripts and Data files.
const TEMPLATES: &str = "crates/yuanme-engine/tests/sites/eyuan-sidecars";

/// The Markdown files whose math the Engine renders whole (their
/// snapshots in yuanme-render show no error), so each of their `$$` blocks
/// renders on its own too.
const MATH: &[&str] = &[
    "al-folio/_posts--2015-10-20-math.md",
    "d2l/chapter_appendix-mathematics-for-deep-learning--single-variable-calculus.md",
    "d2l/chapter_linear-regression--linear-regression.md",
    "halo2-book/book--src--background--fields.md",
    "halo2-book/book--src--background--polynomials.md",
    "halo2-book/book--src--design--proving-system--permutation.md",
    "ichunyu/content--posts--signal--iir-design.md",
    "marc-b-reynolds/_posts--2019-12-10-GammaRamp.md",
    "quarto-cli/tests--docs--smoke-all--crossrefs--equations--equations-alt.md",
];

/// The image fixtures: photos of 1600×1066, 1440×1080 turned by EXIF
/// orientation, and 720×1280 in Display P3, as a blog's photos are.
const PHOTOS: &[&str] = &["beach.jpg", "park.jpg", "fallingwater.jpg"];

/// The corpus's languages that are common in programmers' Posts, as
/// Shiki names them, with the line comment that makes each block unique.
const LANGUAGES: &[(&[&str], &str, &str)] = &[
    (&["c"], "c", "//"),
    (&["cpp", "c++"], "cpp", "//"),
    (&["rust"], "rust", "//"),
    (&["js", "javascript"], "javascript", "//"),
    (&["go"], "go", "//"),
    (&["d"], "d", "//"),
    (&["r"], "r", "#"),
    (&["bash", "sh", "shell"], "bash", "#"),
    (&["python", "jupyter-python"], "python", "#"),
    (&["julia"], "julia", "#"),
    (&["toml", "conf-toml"], "toml", "#"),
    (&["yaml"], "yaml", "#"),
    (&["nim"], "nim", "#"),
    (&["emacs-lisp", "elisp"], "emacs-lisp", ";;"),
    (&["haskell"], "haskell", "--"),
    (&["lua"], "lua", "--"),
];

/// Org tags can't hold a hyphen.
const TAGS: &[&str] = &[
    "rust",
    "emacs",
    "org",
    "math",
    "algebra",
    "calculus",
    "probability",
    "statistics",
    "geometry",
    "topology",
    "compilers",
    "parsers",
    "types",
    "haskell",
    "python",
    "julia",
    "lisp",
    "shell",
    "linux",
    "networking",
    "databases",
    "graphics",
    "shaders",
    "audio",
    "signals",
    "crypto",
    "proofs",
    "logic",
    "category",
    "physics",
    "notes",
    "tools",
    "editors",
    "testing",
    "performance",
    "memory",
    "concurrency",
    "web",
    "design",
    "life",
];

const SERIES: &[&str] = &[
    "advent",
    "zero-to-parser",
    "field-notes",
    "proof-club",
    "dotfiles",
    "reading-sicp",
    "gpu-diary",
    "type-theory",
    "small-tools",
    "retro",
];

/// Writes the Scale Site into `out`, which must not exist yet.
pub fn generate(out: &Path) -> io::Result<()> {
    let root = crate::root();
    copy_dir(&root.join(TEMPLATES), out)?;
    fs::remove_dir_all(out.join("content/posts"))?;
    // The fixture lacks the Reference site's home page and the files its
    // photos and a project's hero name. Left broken, every Page would warn,
    // and the Gates would time the warnings' suggestions.
    let mut config = fs::read_to_string(out.join("yuanme.toml"))?;
    config.push_str("\n[routes.home]\npath = \"/\"\ntemplate = \"about.html\"\n");
    config.push_str("\n[feeds.rss]\ncollections = [\"posts\"]\npath = \"/rss.xml\"\n");
    // The Reference site's outputs, PhotoPile's (#15's spike): #15's 75 ms
    // per output was measured on them, not on the default widths.
    config.push_str(
        "\n[images]\nwidths = [160, 240, 320, 480, 640]\nformats = [\"avif\", \"webp\"]\n",
    );
    fs::write(out.join("yuanme.toml"), config)?;
    fs::write(out.join("data/photos.yaml"), "[]\n")?;
    let lrle = fs::read_to_string(out.join("content/projects/lrle.md"))?;
    let lrle = lrle.replace("hero:\n  url: /img/lrle.png\n", "");
    fs::write(out.join("content/projects/lrle.md"), lrle)?;

    let pool = Pool::read(&root.join("corpus"))?;
    let photos: Vec<Vec<u8>> = PHOTOS
        .iter()
        .map(|p| fs::read(root.join("crates/yuanme-assets/fixtures/images").join(p)))
        .collect::<io::Result<_>>()?;
    let mut rng = Rng(0x0005_ca1e_5173);
    let (mut md, mut org) = (0, 0);
    let mut flat: Vec<(String, bool)> = Vec::new();
    for i in 0..POSTS {
        let is_org = i % 10 >= 7;
        // Its place among the Posts of its format, so each format gets its
        // share of math and images.
        let j = if is_org { org } else { md };
        if is_org {
            org += 1;
        } else {
            md += 1;
        }
        let post = Post {
            i,
            org: is_org,
            math: j % 5 == 1,
            photo: j % 10 == 4,
        };
        let slug = format!("{}-{:04}", TAGS[i % TAGS.len()], i + 1);
        let ext = if is_org { "org" } else { "md" };
        let (path, up) = match post.photo {
            true => (format!("content/posts/{slug}/index.{ext}"), "../"),
            false => (format!("content/posts/{slug}.{ext}"), ""),
        };
        // A link to an earlier Post, by its file, as Authors write them.
        let earlier = match flat.len() {
            0 => None,
            n => Some(flat[rng.below(n)].clone()),
        };
        let link = earlier.map(|(s, o)| format!("{up}{s}.{}", if o { "org" } else { "md" }));
        let text = post.write(&pool, &mut rng, link.as_deref());
        let path = out.join(path);
        fs::create_dir_all(path.parent().expect("inside content/posts"))?;
        fs::write(&path, text)?;
        if post.photo {
            let photo = &photos[i % photos.len()];
            fs::write(path.with_file_name("photo.jpg"), unique_jpeg(photo, i))?;
        } else {
            flat.push((slug, is_org));
        }
    }
    Ok(())
}

/// The same pixels with a comment after the JPEG's start marker, so every
/// file has its own hash and is encoded, as distinct photos would be.
fn unique_jpeg(jpeg: &[u8], i: usize) -> Vec<u8> {
    let comment = format!("yuanme Scale Site photo {i}");
    let len = u16::try_from(comment.len() + 2).expect("a short comment");
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xFE]);
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(comment.as_bytes());
    out.extend_from_slice(&jpeg[2..]);
    out
}

struct Post {
    i: usize,
    org: bool,
    math: bool,
    photo: bool,
}

impl Post {
    fn write(&self, pool: &Pool, rng: &mut Rng, link: Option<&str>) -> String {
        let i = self.i;
        let tags: Vec<&str> = {
            let mut tags = vec![TAGS[i % TAGS.len()]];
            for _ in 0..rng.below(3) {
                let tag = TAGS[rng.below(TAGS.len())];
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            tags
        };
        let series = (i % 10 == 9).then(|| SERIES[(i / 10) % SERIES.len()]);
        let title = format!("{} notes {}", capitalized(tags[0]), i + 1);
        let intro = pool.paragraph(rng);
        let description = cut(intro, 140).replace('"', "");
        // Ten years of Posts, a few days apart.
        let date = format!(
            "{}-{:02}-{:02}",
            2015 + i / 100,
            (i / 8) % 12 + 1,
            i % 28 + 1
        );

        let mut out = String::new();
        if self.org {
            out.push_str(&format!("#+TITLE: {title}\n#+DATE: {date}\n"));
            out.push_str(&format!("#+DESCRIPTION: {description}\n"));
            out.push_str(&format!("#+FILETAGS: :{}:\n", tags.join(":")));
            if let Some(series) = series {
                out.push_str(&format!("#+SERIES: {series}\n"));
            }
        } else {
            out.push_str(&format!(
                "---\ntitle: \"{title}\"\ndescription: \"{description}\"\ndate: {date}\ntags: [{}]\n",
                tags.join(", ")
            ));
            if let Some(series) = series {
                out.push_str(&format!("series: {series}\n"));
            }
            out.push_str("---\n");
        }
        out.push('\n');
        out.push_str(&self.paragraph(intro, rng));
        if self.photo {
            let alt = cut(pool.paragraph(rng), 60).replace('"', "");
            out.push_str(&match self.org {
                true => format!("#+CAPTION: {alt}\n[[file:photo.jpg]]\n\n"),
                false => format!("![{alt}](photo.jpg)\n\n"),
            });
        }

        // Code blocks a Post: 3 to 7, 5 on average. Sections carry them
        // first, then prose fills the Post to its size.
        let blocks = 3 + rng.below(5);
        let formulas = if self.math { 1 + rng.below(3) } else { 0 };
        let size = 8 * 1024 + rng.below(7 * 1024);
        let (mut code, mut math) = (0, 0);
        for n in 1.. {
            let carries = code < blocks || math < formulas;
            if !carries && out.len() >= size {
                break;
            }
            let mut section = String::new();
            let heading = cut(pool.paragraph(rng), 40);
            let heading = heading.trim_end_matches(|c: char| !c.is_alphanumeric());
            section.push_str(&match self.org {
                true => format!("* {n}. {heading}\n\n"),
                false => format!("## {n}. {heading}\n\n"),
            });
            section.push_str(&self.paragraph(pool.paragraph(rng), rng));
            if n == 2
                && let Some(link) = link
            {
                section.push_str(&match self.org {
                    true => format!("As [[file:{link}][an earlier Post]] said, and [[https://example.com/{i}][this page]] shows.\n\n"),
                    false => format!("As [an earlier Post]({link}) said, and [this page](https://example.com/{i}) shows.\n\n"),
                });
            }
            if n % 3 == 0 {
                for _ in 0..3 + rng.below(3) {
                    section.push_str(&format!("- {}\n", cut(pool.paragraph(rng), 80)));
                }
                section.push('\n');
            }
            if math < formulas {
                math += 1;
                section.push_str(&self.math(pool, rng));
            }
            if code < blocks {
                code += 1;
                let (lang, comment, body) = pool.code(rng);
                let label = format!("{comment} Post {}, block {code}", i + 1);
                section.push_str(&match self.org {
                    true => format!("#+begin_src {lang}\n{label}\n{body}#+end_src\n\n"),
                    false => format!("```{lang}\n{label}\n{body}```\n\n"),
                });
            }
            if !carries && out.len() + section.len() > 16 * 1024 {
                break;
            }
            out.push_str(&section);
        }
        out
    }

    /// `text` with one word in bold and another as code.
    fn paragraph(&self, text: &str, rng: &mut Rng) -> String {
        let mut words: Vec<String> = text.split(' ').map(String::from).collect();
        let n = words.len();
        for (k, at) in [rng.below(n), rng.below(n)].into_iter().enumerate() {
            let word = &words[at];
            if word.len() < 3 || !word.chars().all(|c| c.is_ascii_alphanumeric()) {
                continue;
            }
            words[at] = match (k, self.org) {
                (0, true) => format!("*{word}*"),
                (0, false) => format!("**{word}**"),
                (_, true) => format!("~{word}~"),
                (_, false) => format!("`{word}`"),
            };
        }
        words.join(" ") + "\n\n"
    }

    /// A display formula, and a paragraph with one inline.
    fn math(&self, pool: &Pool, rng: &mut Rng) -> String {
        let display = pool.formula(rng, false);
        let inline = pool.formula(rng, true);
        let lead = cut(pool.paragraph(rng), 200);
        match self.org {
            true => format!(
                "\\[ {} \\]\n\n{lead}, where \\({inline}\\).\n\n",
                display.replace('\n', " ")
            ),
            false => format!("$$\n{display}\n$$\n\n{lead}, where ${inline}$.\n\n"),
        }
    }
}

/// What the corpus offers.
struct Pool {
    paragraphs: Vec<String>,
    /// By language: (Shiki name, line comment, blocks).
    code: Vec<(&'static str, &'static str, Vec<String>)>,
    formulas: Vec<String>,
}

impl Pool {
    fn read(corpus: &Path) -> io::Result<Pool> {
        let mut files = Vec::new();
        for dir in ["md", "org"] {
            files.extend(walk(&corpus.join(dir))?);
        }
        files.sort();
        let mut pool = Pool {
            paragraphs: Vec::new(),
            code: LANGUAGES
                .iter()
                .map(|(_, l, c)| (*l, *c, Vec::new()))
                .collect(),
            formulas: Vec::new(),
        };
        for path in &files {
            let text = fs::read_to_string(path)?;
            pool.scan(&text);
        }
        for file in MATH {
            let text = fs::read_to_string(corpus.join("md").join(file))?;
            pool.formulas.extend(display_math(&text));
        }
        pool.paragraphs.sort();
        pool.paragraphs.dedup();
        pool.code.retain(|(_, _, blocks)| !blocks.is_empty());
        Ok(pool)
    }

    /// Plain paragraphs and code blocks, from Markdown or Org.
    fn scan(&mut self, text: &str) {
        let mut lines = text.lines().peekable();
        let mut paragraph: Vec<&str> = Vec::new();
        while let Some(line) = lines.next() {
            let opening = line
                .strip_prefix("```")
                .map(|rest| (rest, "```"))
                .or_else(|| {
                    let lower = line.trim_start();
                    let rest = lower
                        .strip_prefix("#+begin_src ")
                        .or_else(|| lower.strip_prefix("#+BEGIN_SRC "))?;
                    Some((rest, "#+end_src"))
                });
            if let Some((rest, close)) = opening {
                self.keep_paragraph(&paragraph);
                paragraph.clear();
                let lang = rest
                    .trim_start_matches('{')
                    .split(|c: char| c.is_whitespace() || c == '}')
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                let mut body = String::new();
                for line in lines.by_ref() {
                    let end = line.trim_start().to_ascii_lowercase();
                    if end.starts_with(close) {
                        break;
                    }
                    body.push_str(line);
                    body.push('\n');
                }
                self.keep_block(&lang, body);
                continue;
            }
            if line.trim().is_empty() {
                self.keep_paragraph(&paragraph);
                paragraph.clear();
            } else {
                paragraph.push(line);
            }
        }
        self.keep_paragraph(&paragraph);
    }

    /// Keeps a paragraph that reads the same in both Source formats: plain
    /// words and punctuation, starting each line, with no markup.
    fn keep_paragraph(&mut self, lines: &[&str]) {
        let plain = |c: char| c.is_ascii_alphanumeric() || " .,;:'\"()?!-".contains(c);
        let ok = !lines.is_empty()
            && lines
                .iter()
                .all(|l| l.starts_with(|c: char| c.is_ascii_alphabetic()) && l.chars().all(plain));
        if !ok {
            return;
        }
        let text = lines.join(" ");
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if (200..=1100).contains(&text.len()) && text.ends_with('.') {
            self.paragraphs.push(text);
        }
    }

    /// Keeps a code block both Source formats hold as it is: no line an
    /// Org block would end at or unescape, and none a fence would close.
    fn keep_block(&mut self, lang: &str, body: String) {
        let Some(at) = LANGUAGES
            .iter()
            .position(|(names, _, _)| names.contains(&lang))
        else {
            return;
        };
        let lines = body.lines().count();
        let ok = (2..=30).contains(&lines)
            && body.len() <= 1100
            && body.lines().all(|l| {
                !(l.starts_with('*') || l.starts_with(',') || l.trim_start().starts_with("#+"))
                    && !l.contains("```")
            });
        if ok {
            self.code[at].2.push(body);
        }
    }

    fn paragraph(&self, rng: &mut Rng) -> &str {
        &self.paragraphs[rng.below(self.paragraphs.len())]
    }

    /// A language first, so the corpus's thousand Emacs Lisp blocks don't
    /// make every Post a Lisp one.
    fn code(&self, rng: &mut Rng) -> (&'static str, &'static str, &str) {
        let (lang, comment, blocks) = &self.code[rng.below(self.code.len())];
        (lang, comment, &blocks[rng.below(blocks.len())])
    }

    fn formula(&self, rng: &mut Rng, inline: bool) -> &str {
        loop {
            let f = &self.formulas[rng.below(self.formulas.len())];
            if !inline || (f.len() <= 60 && !f.contains('\n')) {
                return f;
            }
        }
    }
}

/// The `$$` blocks of a Markdown file that start a line, as one formula
/// each, without those that need the rest of their file.
fn display_math(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut lines = text.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix("$$") else {
            continue;
        };
        let formula = match rest.split_once("$$") {
            Some((formula, _)) => formula.to_string(),
            None => {
                let mut formula = rest.to_string();
                let mut closed = false;
                for line in lines.by_ref() {
                    if let Some((last, _)) = line.split_once("$$") {
                        formula.push('\n');
                        formula.push_str(last);
                        closed = true;
                        break;
                    }
                    formula.push('\n');
                    formula.push_str(line);
                }
                if !closed {
                    break;
                }
                formula
            }
        };
        let formula = formula.trim();
        let alone = [
            "\\label",
            "\\tag",
            "\\ref",
            "\\eqref",
            "\\newcommand",
            "\\def",
            "$",
        ]
        .iter()
        .all(|t| !formula.contains(t));
        if alone && (3..=400).contains(&formula.len()) && !formula.contains("\n\n") {
            out.push(formula.to_string());
        }
    }
    out
}

fn walk(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            out.extend(walk(&path)?);
        } else if path.extension().is_some_and(|e| e == "md" || e == "org") {
            out.push(path);
        }
    }
    Ok(out)
}

fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let path = entry?.path();
        let target = to.join(path.file_name().expect("a directory entry"));
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// The first words of `text`, up to about `max` bytes.
fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let end = text[..max].rfind(' ').unwrap_or(max);
    &text[..end]
}

fn capitalized(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_ascii_uppercase().to_string() + chars.as_str())
        .unwrap_or_default()
}

/// SplitMix64: the Site must be the same on every machine, and no crate is
/// needed for that.
struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z % n as u64) as usize
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::generate;

    fn posts(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.extend(posts(&path));
            } else if path.extension().is_some_and(|e| e == "md" || e == "org") {
                out.push(path);
            }
        }
        out
    }

    /// #15's Scale Site: 1,000 Posts of 8–16 KB, 70% Markdown and 30% Org,
    /// 5 code blocks a Post on average, math in 20% of them, 100 images,
    /// and tag, feed and sitemap pages, with the Reference site's Templates.
    #[test]
    fn the_scale_site_is_the_one_15_describes() {
        let dir = tempfile::tempdir().unwrap();
        let root = &dir.path().join("scale");
        generate(root).unwrap();

        let mut formats: BTreeMap<&str, usize> = BTreeMap::new();
        let (mut code, mut math) = (0, 0);
        let mut tags = BTreeSet::new();
        let mut images = BTreeSet::new();
        let all = posts(&root.join("content/posts"));
        for path in &all {
            let text = fs::read_to_string(path).unwrap();
            let size = text.len();
            assert!(
                (8 * 1024..=16 * 1024).contains(&size),
                "{}: {size} bytes",
                path.display()
            );
            let lines = || text.lines().map(str::trim_start);
            let org = path.extension().unwrap() == "org";
            *formats.entry(if org { "org" } else { "md" }).or_default() += 1;
            if org {
                code += lines().filter(|l| l.starts_with("#+begin_src ")).count();
                math += usize::from(lines().any(|l| l.starts_with("\\[")));
                let filetags = lines()
                    .find_map(|l| l.strip_prefix("#+FILETAGS: "))
                    .unwrap();
                tags.extend(
                    filetags
                        .split(':')
                        .filter(|t| !t.is_empty())
                        .map(String::from),
                );
            } else {
                let fences = lines().filter(|l| l.starts_with("```")).count();
                assert_eq!(fences % 2, 0, "{}", path.display());
                code += fences / 2;
                math += usize::from(lines().any(|l| l == "$$"));
                let list = lines().find_map(|l| l.strip_prefix("tags: [")).unwrap();
                tags.extend(list.trim_end_matches(']').split(", ").map(String::from));
            }
            // A page bundle's photo, which its Post shows.
            let photo = path.with_file_name("photo.jpg");
            if photo.exists() {
                assert!(text.contains("photo.jpg"), "{}", path.display());
                images.insert(fs::read(photo).unwrap());
            }
        }
        assert_eq!(formats, BTreeMap::from([("md", 700), ("org", 300)]));
        let per_post = code as f64 / all.len() as f64;
        assert!(
            (4.8..=5.2).contains(&per_post),
            "{per_post} code blocks a Post"
        );
        assert_eq!(math, 200);
        // Distinct files, or the cache would encode one of them once.
        assert_eq!(images.len(), 100);
        assert!(images.iter().all(|i| i.starts_with(&[0xFF, 0xD8])));
        assert!(tags.len() >= 20, "{tags:?}");

        let config = fs::read_to_string(root.join("yuanme.toml")).unwrap();
        assert!(config.contains("[feeds.rss]"));
        // The Reference site's Templates.
        assert!(root.join("templates/layouts/post.html").is_file());
    }
}
