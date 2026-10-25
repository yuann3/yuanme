use std::collections::BTreeSet;
use std::fmt::Write as _;

use read_fonts::collections::IntSet;
use read_fonts::tables::cmap::MapVariant;
use read_fonts::tables::math::{MathConstant, MathKernCorner, StretchAxis};
use read_fonts::types::{GlyphId, Tag};
use read_fonts::{FontRef, TableProvider};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};

use super::{NCM_MATH, subset, woff2};

/// Primes and digits, which `ssty` reaches; delimiters, roots, sums and
/// integrals, which `MATH` stretches; accents; arrows; italic and
/// calligraphic letters, the second with both variation sequences; and the
/// characters browsers measure.
const CHARS: &str =
    "x𝑥ℎ𝑓′″0123456789()[]{}|‖⟨⟩√∑∫∏^~\u{302}\u{303}\u{20D7}¯→⟶=+−<≤αβΓΔ𝒜ℒ\u{FE00}\u{FE01} \u{a0}";

#[test]
fn every_kept_character_draws_and_stretches_as_in_the_font() {
    let chars: BTreeSet<char> = CHARS.chars().collect();
    let out = subset(NCM_MATH, &chars);
    let (full, part) = (FontRef::new(NCM_MATH).unwrap(), FontRef::new(&out).unwrap());

    let glyphs = part.maxp().unwrap().num_glyphs();
    assert!(glyphs < 600, "{glyphs} glyphs: not subset");
    assert_eq!(part.charmap().map('Q'), None);
    let constants = |font: &FontRef| -> Vec<i32> {
        let constants = font.math().unwrap().math_constants().unwrap();
        (0..)
            .map_while(MathConstant::new)
            .map(|c| constants.constant(c))
            .collect()
    };
    assert_eq!(constants(&full), constants(&part));
    for c in chars {
        let (a, b) = (full.charmap().map(c), part.charmap().map(c));
        let a = a.unwrap_or_else(|| panic!("the font has no {c:?}"));
        let b = b.unwrap_or_else(|| panic!("the subset has no {c:?}"));
        assert_eq!(describe(&full, a), describe(&part, b), "{c:?}");
        for selector in [0xFE00_u32, 0xFE01] {
            let variant = |font: &FontRef, base| match font.charmap().map_variant(c, selector)? {
                MapVariant::UseDefault => Some(base),
                MapVariant::Variant(g) => Some(g),
            };
            assert_eq!(
                variant(&full, a).map(|g| describe(&full, g)),
                variant(&part, b).map(|g| describe(&part, g)),
                "{c:?} with U+{selector:X}"
            );
        }
    }
}

#[test]
fn the_subset_keeps_the_fonts_notices() {
    let out = subset(NCM_MATH, &BTreeSet::from(['x']));
    let font = FontRef::new(&out).unwrap();
    let names: Vec<String> = [0, 1, 10, 13, 14]
        .into_iter()
        .map(|id| {
            let id = read_fonts::types::NameId::new(id);
            font.localized_strings(id)
                .english_or_first()
                .unwrap()
                .to_string()
        })
        .collect();
    assert!(names[0].contains("Antonis Tsolomitis"), "{names:?}");
    assert_eq!(names[1], "yuanme NCM Math");
    assert!(names[2].contains("math-core-fonts"), "{names:?}");
    assert!(names[3].contains("GUST Font License"), "{names:?}");
    assert!(names[4].starts_with("https://"), "{names:?}");
}

#[test]
fn woff2_holds_the_same_tables() {
    let out = subset(NCM_MATH, &BTreeSet::from(['x', '∑']));
    let packed = woff2(&out);
    assert_eq!(&packed[..4], b"wOF2");
    assert!(
        packed.len() < out.len() / 2,
        "{} of {}",
        packed.len(),
        out.len()
    );
}

/// Everything a browser reads of a glyph: its outline and advance, its
/// MATH data, and the same of each glyph `ssty` or MATH reaches from it.
fn describe(font: &FontRef, glyph: GlyphId) -> String {
    let mut out = shape(font, glyph);
    let math = font.math().unwrap();
    let info = math.math_glyph_info().unwrap();
    let italic = info
        .math_italics_correction_info()
        .and_then(|t| t.ok()?.correction(glyph));
    let accent = info
        .math_top_accent_attachment()
        .and_then(|t| t.ok()?.attachment(glyph));
    let extended = info.is_extended_shape(glyph);
    write!(
        out,
        " italic {italic:?} accent {accent:?} extended {extended}"
    )
    .unwrap();
    if let Some(Ok(kerns)) = info.math_kern_info() {
        for corner in [
            MathKernCorner::TopRight,
            MathKernCorner::TopLeft,
            MathKernCorner::BottomRight,
            MathKernCorner::BottomLeft,
        ] {
            if let Some(kern) = kerns.kern(glyph, corner) {
                let entries: Vec<_> = kern.entries_for_ppem(0).collect();
                write!(out, " kern {entries:?}").unwrap();
            }
        }
    }
    let variants = math.math_variants().unwrap();
    for axis in [StretchAxis::Vertical, StretchAxis::Horizontal] {
        let Some(c) = variants.glyph_construction(glyph, axis) else {
            continue;
        };
        for v in c.math_glyph_variant_records() {
            let size = v.advance_measurement();
            write!(
                out,
                "\n  {axis:?} {size}: {}",
                shape(font, v.variant_glyph().into())
            )
            .unwrap();
        }
        if let Some(Ok(assembly)) = c.glyph_assembly() {
            for p in assembly.part_records() {
                let (start, end) = (p.start_connector_length(), p.end_connector_length());
                let (full, flags) = (p.full_advance(), p.part_flags().bits());
                let part = shape(font, p.glyph_id().into());
                write!(out, "\n  part {start} {end} {full} {flags}: {part}").unwrap();
            }
        }
    }
    let gsub = font.gsub().unwrap();
    let ssty = IntSet::from([Tag::new(b"ssty")]);
    let features = gsub
        .collect_features(&IntSet::all(), &IntSet::all(), &ssty)
        .unwrap();
    let lookups = gsub.collect_lookups(&features).unwrap();
    let mut reached = IntSet::from([glyph]);
    gsub.closure_glyphs(&lookups, &mut reached).unwrap();
    let mut forms: Vec<String> = reached
        .iter()
        .filter(|&g| g != glyph)
        .map(|g| shape(font, g))
        .collect();
    forms.sort();
    for form in forms {
        write!(out, "\n  ssty: {form}").unwrap();
    }
    out
}

fn shape(font: &FontRef, glyph: GlyphId) -> String {
    let advance = font
        .glyph_metrics(Size::unscaled(), LocationRef::default())
        .advance_width(glyph);
    let mut pen = Pen(format!("advance {advance:?}"));
    font.outline_glyphs()
        .get(glyph)
        .unwrap()
        .draw(
            DrawSettings::unhinted(Size::unscaled(), LocationRef::default()),
            &mut pen,
        )
        .unwrap();
    pen.0
}

struct Pen(String);

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        write!(self.0, " M{x},{y}").unwrap();
    }
    fn line_to(&mut self, x: f32, y: f32) {
        write!(self.0, " L{x},{y}").unwrap();
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        write!(self.0, " Q{cx0},{cy0},{x},{y}").unwrap();
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        write!(self.0, " C{cx0},{cy0},{cx1},{cy1},{x},{y}").unwrap();
    }
    fn close(&mut self) {
        self.0.push_str(" Z");
    }
}
