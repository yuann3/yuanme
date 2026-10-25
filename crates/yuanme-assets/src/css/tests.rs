use std::fmt;

use super::*;

/// Files by path; `@import`s resolve against the importing file's folder,
/// and `https:` is External.
struct Files(&'static [(&'static str, &'static str)]);

#[derive(Debug, PartialEq, Eq)]
struct Missing(String);

impl fmt::Display for Missing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no {}", self.0)
    }
}

impl std::error::Error for Missing {}

impl Project for Files {
    type Error = Missing;

    fn read(&self, path: &str) -> Result<String, Missing> {
        self.0
            .iter()
            .find(|(p, _)| *p == path)
            .map(|(_, text)| text.to_string())
            .ok_or_else(|| Missing(path.into()))
    }

    fn import(&self, specifier: &str, from: &str) -> Result<Import, Missing> {
        if specifier.starts_with("https:") {
            return Ok(Import::External(specifier.into()));
        }
        let dir = from.rsplit_once('/').map_or("", |(d, _)| d);
        let path = match dir {
            "" => specifier.to_string(),
            _ => format!("{dir}/{specifier}"),
        };
        self.read(&path).map(|_| Import::File(path))
    }
}

fn targets(list: &[(&str, [u32; 3])]) -> BTreeMap<String, [u32; 3]> {
    list.iter().map(|(b, v)| (b.to_string(), *v)).collect()
}

fn code(files: &'static [(&'static str, &'static str)], list: &[(&str, [u32; 3])]) -> String {
    match bundle(&Files(files), files[0].0, &targets(list)) {
        Ok(b) => b.code,
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn imports_are_inlined_and_each_url_names_the_file_it_is_written_in() {
    let files = &[
        (
            "styles/site.css",
            "@import \"https://fonts.example/a.css\";\n@import \"parts/type.css\" screen;\nbody { background: url(bg.png) }\n",
        ),
        (
            "styles/parts/type.css",
            "@font-face {\n  font-family: G;\n  src: url(\"../../fonts/g.woff2\");\n}\nh1 { color: red }\n",
        ),
    ];
    let bundle = bundle(&Files(files), "styles/site.css", &targets(&[])).unwrap();

    let urls: Vec<_> = bundle
        .urls
        .iter()
        .map(|u| (u.url.as_str(), u.file.as_str(), u.line))
        .collect();
    assert_eq!(
        urls,
        [
            ("https://fonts.example/a.css", "styles/site.css", 1),
            ("../../fonts/g.woff2", "styles/parts/type.css", 3),
            ("bg.png", "styles/site.css", 3),
        ]
    );
    let mut code = bundle.code.clone();
    for (u, name) in bundle.urls.iter().zip(["EXT", "FONT", "BG"]) {
        code = code.replace(&format!("\"{}\"", u.placeholder), name);
    }
    assert_eq!(
        code,
        "@import EXT;@media screen{@font-face{font-family:G;src:url(FONT)}h1{color:red}}body{background:url(BG)}"
    );
}

#[test]
fn targets_decide_which_fallbacks_and_prefixes_stay() {
    let files = &[(
        "a.css",
        ".a{height:100vh;height:100dvh}::selection{color:red}.b{user-select:none}",
    )];
    // Every target supports dvh from these versions: Chrome 108, Safari 15.4.
    assert_eq!(
        code(files, &[("chrome", [108, 0, 0]), ("safari", [15, 4, 0])]),
        ".a{height:100dvh}::selection{color:red}.b{-webkit-user-select:none;user-select:none}"
    );
    assert_eq!(
        code(files, &[("chrome", [108, 0, 0]), ("safari", [15, 3, 0])]),
        ".a{height:100vh;height:100dvh}::selection{color:red}.b{-webkit-user-select:none;user-select:none}"
    );
    // Firefox before 62 needs `::-moz-selection`.
    assert_eq!(
        code(files, &[("firefox", [61, 0, 0])]),
        ".a{height:100vh;height:100dvh}::-moz-selection{color:red}::selection{color:red}.b{-moz-user-select:none;user-select:none}"
    );
}

#[test]
fn an_error_is_located_in_the_file_it_is_in() {
    let files: &[(&str, &str)] = &[
        ("a.css", "@import \"b.css\";\n"),
        ("b.css", "p { color: red }\n\n  a$b { color: red }\n"),
    ];
    let e = bundle(&Files(files), "a.css", &targets(&[])).err().unwrap();
    assert!(matches!(e.kind, ErrorKind::Css(_)), "{e:?}");
    assert_eq!(e.at.map(|at| (at.file, at.line)), Some(("b.css".into(), 3)));

    let files: &[(&str, &str)] = &[("a.css", "\n@import \"c.css\";\n")];
    let e = bundle(&Files(files), "a.css", &targets(&[])).err().unwrap();
    let ErrorKind::Project(missing) = e.kind else {
        panic!("{e:?}")
    };
    assert_eq!(missing, Missing("c.css".into()));
    assert_eq!(
        e.at,
        Some(Location {
            file: "a.css".into(),
            line: 2,
            column: 1
        })
    );
}
