//! Body images, `image()` and `picture` (#28), built from projects held in
//! memory through the Engine's one seam. The images are small and made
//! here, so encoding them is quick.

use std::collections::BTreeSet;
use std::fmt::Write;
use std::io::Cursor;

use image::codecs::gif::GifEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::{Delay, DynamicImage, Frame, ImageFormat, Rgba, RgbaImage};
use yuanme_engine::{Body, Build, Cache, Inputs, Mode, build};

const CONFIG: &str = "[site]
url = \"https://e.com\"

[collections.posts]
route = \"/blog/{slug}/\"

[images]
widths = [240, 480]
";

/// A photo-like gradient, `width` × `height`, with alpha or without.
fn pixels(width: u32, height: u32, alpha: bool) -> DynamicImage {
    let image = RgbaImage::from_fn(width, height, |x, y| {
        let a = if alpha && x < width / 4 { 0 } else { 255 };
        Rgba([(x * 255 / width) as u8, (y * 255 / height) as u8, 90, a])
    });
    match alpha {
        true => DynamicImage::ImageRgba8(image),
        false => DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(image).to_rgb8()),
    }
}

fn jpeg(width: u32, height: u32) -> Vec<u8> {
    let mut out = Vec::new();
    pixels(width, height, false)
        .write_with_encoder(JpegEncoder::new_with_quality(&mut out, 90))
        .unwrap();
    out
}

fn png(width: u32, height: u32, alpha: bool) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    pixels(width, height, alpha)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn animated_gif() -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = GifEncoder::new(&mut out);
    for red in [0, 255] {
        let frame = RgbaImage::from_pixel(30, 20, Rgba([red, 0, 0, 255]));
        let delay = Delay::from_numer_denom_ms(100, 1);
        encoder
            .encode_frame(Frame::from_parts(frame, 0, 0, delay))
            .unwrap();
    }
    drop(encoder);
    out
}

const SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 40 30\"></svg>";

fn project(files: &[(&str, &[u8])]) -> Inputs {
    let mut inputs = Inputs::default();
    inputs.insert("yuanme.toml", CONFIG.as_bytes());
    inputs.insert(
        "templates/posts/entry.html",
        "{{ entry.content }}\n".as_bytes(),
    );
    for (path, bytes) in files {
        inputs.insert(*path, bytes.to_vec());
    }
    inputs
}

/// Pages as text, and every other file by its size.
fn show(build: &Build) -> String {
    assert!(!build.failed(), "{}", diagnostics(build));
    let mut out = String::new();
    for (path, output) in &build.files {
        let bytes = match &output.body {
            Body::Bytes(bytes) => bytes,
            Body::Input(input) => {
                writeln!(out, "=== {path}\n<copied from {input}>").unwrap();
                continue;
            }
            Body::Cached(_) => unreachable!("a memory cache holds bytes"),
        };
        match path.ends_with(".html") {
            true => writeln!(out, "=== {path}\n{}", std::str::from_utf8(bytes).unwrap()),
            false => writeln!(out, "=== {path}\n<{} bytes>", bytes.len()),
        }
        .unwrap();
    }
    out
}

fn diagnostics(build: &Build) -> String {
    build
        .diagnostics
        .iter()
        .map(|d| d.render(&build.sources, false))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bodies() -> Inputs {
    let sea = "---
title: Sea
---
![A beach at noon](beach.jpg \"The beach\")

![Rooftops at dusk](/photos/roof.png)

![A spinner](spinner.gif)

![The logo](logo.svg)

![A cat](https://example.com/cat.jpg)

![The favicon](/favicon.png)
";
    let plots = "#+title: Plots

#+CAPTION: Sales by month
[[file:plots/sales.png]]

#+ATTR_HTML: :alt Costs by month :class wide
[[file:plots/costs.png]]
";
    project(&[
        ("content/posts/sea/index.md", sea.as_bytes()),
        ("content/posts/sea/beach.jpg", &jpeg(600, 400)),
        ("content/posts/sea/spinner.gif", &animated_gif()),
        ("content/posts/sea/logo.svg", SVG.as_bytes()),
        ("photos/roof.png", &png(300, 200, true)),
        ("static/favicon.png", &png(16, 16, false)),
        ("content/posts/plots.org", plots.as_bytes()),
        ("content/posts/plots/sales.png", &png(200, 100, false)),
        ("content/posts/plots/costs.png", &png(200, 100, false)),
    ])
}

/// Each raster becomes AVIF at every width with a JPEG fallback, or PNG
/// for one with transparency, and nothing is upscaled. The first image of
/// a body is eager. An SVG and an animated GIF are copied with a hash; a
/// remote image and a Static file's URL are left as written.
#[test]
fn body_images_become_pictures() {
    let build = build(&bodies(), &mut Cache::memory("1.0.0"), Mode::Build);
    assert!(build.diagnostics.is_empty(), "{}", diagnostics(&build));
    insta::assert_snapshot!(show(&build));

    let decoded = |prefix: &str, ext: &str| {
        let (_, output) = build
            .files
            .iter()
            .find(|(p, _)| p.starts_with(prefix) && p.ends_with(ext))
            .unwrap();
        let Body::Bytes(bytes) = &output.body else {
            unreachable!("a memory cache holds bytes")
        };
        let image = image::load_from_memory(bytes).unwrap();
        (image.width(), image.height())
    };
    assert_eq!(decoded("assets/beach-240-", ".jpg"), (240, 160));
    // 300 wide: its own width, not 480.
    assert_eq!(decoded("assets/roof-300-", ".png"), (300, 200));
}

/// `yuanme check` encodes nothing, yet writes the Pages a build writes:
/// each variant they name is in `images`, and no link to one is broken.
#[test]
fn a_check_names_every_variant_and_encodes_none() {
    let inputs = bodies();
    let built = build(&inputs, &mut Cache::memory("1.0.0"), Mode::Build);
    let check = Mode::Check { drafts: false };
    let checked = build(&inputs, &mut Cache::memory("1.0.0"), check);
    assert!(checked.diagnostics.is_empty(), "{}", diagnostics(&checked));

    let variants: BTreeSet<&String> = checked.images.keys().collect();
    let left_out: BTreeSet<&String> = built
        .files
        .keys()
        .filter(|p| !checked.files.contains_key(*p))
        .collect();
    assert_eq!(left_out, variants);
    assert_eq!(variants.len(), 12);
    for (path, output) in &checked.files {
        assert_eq!(output.hash, built.files[path].hash, "{path}");
    }
    assert_eq!(
        checked.images.keys().collect::<Vec<_>>(),
        built.images.keys().collect::<Vec<_>>()
    );
}

/// A check reads no variant, so only the builds that encode count towards
/// deleting one (#93): those still delete the variants of an image no
/// Page names any more.
#[test]
fn only_builds_that_encode_age_variants_out() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cache");
    let run = |inputs: &Inputs, mode: Mode| {
        let mut cache = Cache::open(path.clone(), "1.0.0").unwrap();
        let build = build(inputs, &mut cache, mode);
        assert!(!build.failed(), "{}", diagnostics(&build));
        cache.save().unwrap();
        build
    };
    let files = |build: &Build, source: &str| -> Vec<std::path::PathBuf> {
        let large = path.join("large");
        let variants = build.images.values().map(|(variant, _)| variant);
        variants
            .filter(|v| v.source.ends_with(source))
            .map(|v| large.join(v.key.to_hex().as_str()))
            .collect()
    };
    let post = |body: &str| {
        project(&[
            ("content/posts/sea/index.md", body.as_bytes()),
            ("content/posts/sea/beach.jpg", &jpeg(600, 400)),
            ("content/posts/sea/roof.png", &png(300, 200, false)),
        ])
    };
    let both = post("---\ntitle: Sea\n---\n![A beach](beach.jpg)\n\n![A roof](roof.png)\n");
    let beach = post("---\ntitle: Sea\n---\n![A beach](beach.jpg)\n");

    let built = run(&both, Mode::Build);
    let (beaches, roofs) = (files(&built, "beach.jpg"), files(&built, "roof.png"));
    assert_eq!((beaches.len(), roofs.len()), (4, 4));
    // Encoding a variant again writes a new file, which moves this.
    let encoded = std::time::SystemTime::UNIX_EPOCH;
    for file in beaches.iter().chain(&roofs) {
        let file = std::fs::File::options().write(true).open(file).unwrap();
        file.set_modified(encoded).unwrap();
    }
    let modified = |file: &std::path::PathBuf| std::fs::metadata(file).ok()?.modified().ok();

    // One more than the builds a cache entry outlives unread.
    for _ in 0..11 {
        run(&both, Mode::Check { drafts: false });
    }
    run(&both, Mode::Build);
    for file in beaches.iter().chain(&roofs) {
        assert_eq!(modified(file), Some(encoded), "{}", file.display());
    }

    // Ten builds after the last that named it, as for any cache entry.
    for _ in 0..10 {
        run(&beach, Mode::Build);
    }
    assert!(roofs.iter().all(|f| !f.exists()));
    assert!(beaches.iter().all(|f| modified(f) == Some(encoded)));
}

fn rendered(inputs: &Inputs) -> String {
    let build = build(inputs, &mut Cache::memory("1.0.0"), Mode::Build);
    diagnostics(&build)
}

#[test]
fn rendered_body_image_diagnostics() {
    let post = "---
title: Plots
---
![Sales by month](plt.png)

![](plot.png)

![Notes](notes.png)
";
    let org = "#+title: Costs

[[file:plot.png]]
";
    let inputs = project(&[
        ("content/posts/plots/index.md", post.as_bytes()),
        ("content/posts/plots/plot.png", &png(40, 20, false)),
        ("content/posts/plots/notes.png", b"not an image"),
        ("content/posts/costs/index.org", org.as_bytes()),
        ("content/posts/costs/plot.png", &png(40, 20, false)),
    ]);
    insta::assert_snapshot!(rendered(&inputs));
}

/// A source whose header reads but whose pixels don't fails only once it
/// is encoded, which is after render: the error is still at the image.
#[test]
fn an_image_that_fails_to_encode_is_an_error_at_its_reference() {
    let mut truncated = png(40, 20, false);
    truncated.truncate(truncated.len() - 20);
    let post = "---\ntitle: Cut\n---\nSome text.\n\n![A cut image](cut.png)\n";
    let inputs = project(&[
        ("content/posts/cut/index.md", post.as_bytes()),
        ("content/posts/cut/cut.png", &truncated),
    ]);
    let checked = build(
        &inputs,
        &mut Cache::memory("1.0.0"),
        Mode::Check { drafts: false },
    );
    assert!(!checked.failed());
    insta::assert_snapshot!(rendered(&inputs));
}

const HERO: &str = r#"<!doctype html>
<html lang="en">
<head>
<title>{{ entry.title }}</title>
<meta property="og:image" content="{{ absolute_url(image(entry.cover, width=300, format="jpeg").url) }}">
</head>
<body>
{% set hero = image(entry.cover, widths=[240, 480], sizes="(min-width: 768px) 720px, 100vw") %}
{{ hero | picture(alt=entry.title, priority=true, class="hero") }}
<p>{{ hero.width }}×{{ hero.height }}, {{ hero.sources | length }} source</p>
{{ image(entry.cover, widths=[240], formats=["webp"], quality=50) | picture(alt="Small", loading="eager") }}
{{ image("/photos/logo.svg") | picture(alt="", priority=true, attrs={"data-logo": true}) }}
{{ entry.content }}
</body>
</html>
"#;

fn heroes(template: &str) -> Inputs {
    let config = format!("{CONFIG}\n[collections.posts.fields]\ncover = {{ type = \"image\" }}\n");
    let mut inputs = project(&[
        ("templates/posts/entry.html", template.as_bytes()),
        (
            "content/posts/sea/index.md",
            b"---\ntitle: Sea\ncover: beach.jpg\n---\nSome text.\n",
        ),
        ("content/posts/sea/beach.jpg", &jpeg(600, 400)),
        ("photos/logo.svg", SVG.as_bytes()),
    ]);
    inputs.insert("yuanme.toml", config.into_bytes());
    inputs
}

/// `image()` names the outputs of an `image` field or a path, and
/// `picture` writes them; `priority` preloads the image from the head.
/// `width=` with one `format=` is one file, as for `og:image`.
#[test]
fn templates_make_pictures_with_image() {
    let build = build(&heroes(HERO), &mut Cache::memory("1.0.0"), Mode::Build);
    assert!(build.diagnostics.is_empty(), "{}", diagnostics(&build));
    insta::assert_snapshot!(show(&build));
}

#[test]
fn rendered_image_function_diagnostics() {
    let mut inputs = project(&[
        ("content/posts/sea/index.md", b"---\ntitle: Sea\n---\n"),
        ("photos/logo.svg", SVG.as_bytes()),
    ]);
    let mut config = CONFIG.to_string();
    let templates = [
        (
            "remote",
            r#"{{ image("https://example.com/cat.jpg") | picture(alt="A cat") }}"#,
        ),
        (
            "missing",
            r#"{{ image("/photos/lgo.svg") | picture(alt="") }}"#,
        ),
        (
            "both",
            r#"{{ image("/photos/logo.svg", width=240, widths=[480]) | picture(alt="") }}"#,
        ),
        (
            "no-alt",
            r#"{{ image("/photos/logo.svg") | picture(class="logo") }}"#,
        ),
        (
            "not-an-image",
            r#"{{ "/photos/logo.svg" | picture(alt="") }}"#,
        ),
        (
            "option",
            r#"{{ image("/photos/logo.svg", widht=240) | picture(alt="", loading="soon") }}"#,
        ),
        ("field", r#"{{ image("/photos/logo.svg").widht }}"#),
    ];
    for (name, text) in templates {
        writeln!(
            config,
            "[routes.{name}]\npath = \"/{name}/\"\ntemplate = \"{name}.html\""
        )
        .unwrap();
        inputs.insert(
            format!("templates/{name}.html"),
            format!("{text}\n").into_bytes(),
        );
    }
    inputs.insert("yuanme.toml", config.into_bytes());
    insta::assert_snapshot!(rendered(&inputs));
}

/// Bytes that no encoder shrinks, so a file of them is `len` bytes.
fn noise(len: usize) -> Vec<u8> {
    let mut x: u32 = 0x9e37_79b9;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

/// An animated GIF over 50 KB, since its frames are noise.
fn heavy_gif() -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = GifEncoder::new(&mut out);
    for seed in [0, 1] {
        let bytes = noise(4 * 160 * 160 + seed);
        let frame = RgbaImage::from_fn(160, 160, |x, y| {
            let i = (4 * (y * 160 + x)) as usize + seed;
            Rgba([bytes[i], bytes[i + 1], bytes[i + 2], 255])
        });
        let delay = Delay::from_numer_denom_ms(100, 1);
        encoder
            .encode_frame(Frame::from_parts(frame, 0, 0, delay))
            .unwrap();
    }
    drop(encoder);
    assert!(out.len() > 50_000);
    out
}

/// #15's rule: a raster over 50 KB that a Page loads as it is warns, at
/// the reference, unless `allow_unoptimized` lists it. 50,000 bytes is
/// not over; a link the visitor follows loads nothing.
#[test]
fn rendered_unoptimized_diagnostics() {
    let template = r#"<head><link rel="icon" href="/favicon.png"><link rel="stylesheet" href="/styles/site.css"></head>
<img src="/photos/big.jpg" alt=""><img src="/edge.png" alt=""><img src="/over.png" alt="">
<a href="/download.jpg">Download</a>
{{ entry.content }}
"#;
    let big = noise(60_000);
    let mut inputs = project(&[
        ("templates/posts/entry.html", template.as_bytes()),
        (
            "content/posts/sea/index.md",
            b"---\ntitle: Sea\n---\n![A spinner](spinner.gif)\n",
        ),
        ("content/posts/sea/spinner.gif", &heavy_gif()),
        ("static/photos/big.jpg", &big),
        ("static/edge.png", &noise(50_000)),
        ("static/over.png", &noise(50_001)),
        ("static/download.jpg", &big),
        ("static/favicon.png", &big),
        (
            "styles/site.css",
            b"body {\n  background: url(paper.webp);\n}\n",
        ),
        ("styles/paper.webp", &noise(70_000)),
    ]);
    let config = format!("{CONFIG}allow_unoptimized = [\"/favicon.png\"]\n");
    inputs.insert("yuanme.toml", config.into_bytes());
    insta::assert_snapshot!(rendered(&inputs));
}
