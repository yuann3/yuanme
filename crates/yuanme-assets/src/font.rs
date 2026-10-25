//! The math font, subset to the characters a Site's math shows and
//! compressed to WOFF2 (#29, #52).
//!
//! No one crate subsets a CFF math font. skera writes every table but `CFF `
//! and `MATH`, which it drops; allsorts writes `CFF `; `MATH` is subset here.
//! Each is given the glyphs of one closure, so their glyph ids agree.

#[cfg(test)]
mod tests;

use std::collections::BTreeSet;

use allsorts::binary::read::ReadScope;
use allsorts::binary::write::{WriteBinary, WriteBuffer};
use allsorts::cff::CFF;
use read_fonts::collections::IntSet;
use read_fonts::tables::math::Math;
use read_fonts::types::{GlyphId, GlyphId16, NameId, Tag};
use read_fonts::{FontRef, TableProvider};
use skera::{DEFAULT_LAYOUT_FEATURES, Plan, SubsetFlags};
use write_fonts::FontBuilder;
use write_fonts::from_obj::ToOwnedTable;
use write_fonts::tables::layout::CoverageTable;
use write_fonts::tables::math as owned;

/// yuanme NCM Math: New Computer Modern Math, patched by math-core-fonts
/// and renamed, as `font/NOTICE` says.
pub const NCM_MATH: &[u8] = include_bytes!("font/yuanme-ncm-math.otf");

const MATH: Tag = Tag::new(b"MATH");
const CFF_: Tag = Tag::new(b"CFF ");

/// `font` with only the glyphs `chars` need: theirs, what GSUB reaches from
/// them, such as Firefox's `ssty` script forms, and what `MATH` reaches, its
/// size variants and assembly parts. Every kept glyph keeps all its `MATH`
/// data, where hb-subset keeps less for glyphs only GSUB reaches (#17).
///
/// Panics if `font` isn't a CFF font with a `MATH` table: it is always the
/// Engine's own.
pub fn subset(font: &[u8], chars: &BTreeSet<char>) -> Vec<u8> {
    let font = FontRef::new(font).expect("the math font parses");
    let math = font.math().expect("the math font has MATH");
    let unicodes: IntSet<u32> = chars.iter().map(|&c| u32::from(c)).collect();
    // skera closes over cmap and GSUB; MATH's glyphs can reach more through
    // GSUB, and those more through MATH, until nothing changes.
    let mut gids = IntSet::empty();
    let plan = loop {
        let plan = plan(&font, &gids, &unicodes);
        let mut kept: IntSet<GlyphId> = plan.old_to_new_glyph_mapping().map(|(o, _)| o).collect();
        let before = kept.len();
        math_closure(&math, &mut kept);
        if kept.len() == before {
            break plan;
        }
        gids = kept;
    };
    // allsorts and MATH number the glyphs as skera does: in their old order.
    let mut pairs: Vec<(GlyphId, GlyphId)> = plan.old_to_new_glyph_mapping().collect();
    pairs.sort();
    let old: Vec<GlyphId> = pairs.iter().map(|&(o, _)| o).collect();
    assert!(pairs.iter().zip(0..).all(|(&(_, n), i)| n.to_u32() == i));
    let new_id = |g: GlyphId| -> Option<GlyphId16> {
        old.binary_search(&g).ok().map(|i| GlyphId16::new(i as u16))
    };

    let tables = skera::subset_font(&font, &plan).expect("skera subsets the math font");
    let tables = FontRef::new(&tables).expect("skera writes a font");
    let cff = cff(&font, &old);
    let math = subset_math(&math, &new_id);
    let out = FontBuilder::new()
        .add_raw(CFF_, cff)
        .add_raw(MATH, math)
        .copy_missing_tables(tables)
        .build();

    let subset = FontRef::new(&out).expect("the subset parses");
    let cmap = font.cmap().expect("the math font has a cmap");
    let subset_cmap = subset.cmap().expect("skera writes a cmap");
    for &c in chars {
        if let Some(g) = cmap.map_codepoint(c) {
            let kept = subset_cmap.map_codepoint(c);
            assert_eq!(kept, new_id(g).map(GlyphId::from), "{c:?}");
        }
    }
    out
}

/// The font compressed to WOFF2 at Brotli quality 11.
pub fn woff2(otf: &[u8]) -> Vec<u8> {
    ttf2woff2::encode(otf, ttf2woff2::BrotliQuality::default()).expect("a font encodes")
}

fn plan(font: &FontRef, gids: &IntSet<GlyphId>, unicodes: &IntSet<u32>) -> Plan {
    let mut features: IntSet<Tag> = DEFAULT_LAYOUT_FEATURES.iter().copied().collect();
    // Script-size forms, which Firefox applies, and dotless forms under
    // accents, as MathML Core asks.
    features.extend([Tag::new(b"ssty"), Tag::new(b"dtls")]);
    // Its copyright, description and licence too (#27).
    let names: IntSet<NameId> = [0, 1, 2, 3, 4, 5, 6, 10, 13, 14, 16, 17]
        .into_iter()
        .map(NameId::new)
        .collect();
    Plan::new(
        gids,
        unicodes,
        font,
        SubsetFlags::default(),
        &IntSet::empty(),
        &IntSet::all(),
        &features,
        &names,
        &[0x0409].into_iter().collect(),
    )
}

/// Adds every size variant and assembly part of the glyphs in `glyphs`.
fn math_closure(math: &Math, glyphs: &mut IntSet<GlyphId>) {
    let variants = math.math_variants().expect("MathVariants parses");
    let mut found = Vec::new();
    for (coverage, constructions) in [
        (
            variants.vert_glyph_coverage(),
            variants.vert_glyph_constructions(),
        ),
        (
            variants.horiz_glyph_coverage(),
            variants.horiz_glyph_constructions(),
        ),
    ] {
        let Some(Ok(coverage)) = coverage else {
            continue;
        };
        for (glyph, construction) in coverage.iter().zip(constructions.iter()) {
            if !glyphs.contains(glyph) {
                continue;
            }
            let construction = construction.expect("a MATH construction parses");
            for v in construction.math_glyph_variant_records() {
                found.push(GlyphId::from(v.variant_glyph()));
            }
            if let Some(assembly) = construction.glyph_assembly().transpose().expect("parses") {
                for part in assembly.part_records() {
                    found.push(GlyphId::from(part.glyph_id()));
                }
            }
        }
    }
    glyphs.extend(found);
}

/// `CFF ` with only the glyphs `old` lists, in order. Name-keyed, as the
/// font is: allsorts' CID conversion moved glyphs by a fraction of a pixel
/// (#29).
fn cff(font: &FontRef, old: &[GlyphId]) -> Vec<u8> {
    let data = font.table_data(CFF_).expect("the math font is CFF");
    let cff = ReadScope::new(data.as_bytes())
        .read::<CFF>()
        .expect("allsorts reads the CFF");
    let ids: Vec<u16> = old.iter().map(|g| g.to_u32() as u16).collect();
    // allsorts keeps the whole String INDEX, which is mostly glyph names:
    // 110 KB in New Computer Modern. The dropped glyphs' names are emptied;
    // an empty entry costs its offset.
    let keep: BTreeSet<u16> = ids.iter().copied().collect();
    let charset = &cff.fonts[0].charset;
    let dropped: Vec<u16> = (0..cff.fonts[0].char_strings_index.len() as u16)
        .filter(|g| !keep.contains(g))
        .filter_map(|g| charset.id_for_glyph(g))
        .collect();
    let mut subset: CFF = cff
        .subset(&ids, false)
        .expect("allsorts subsets the CFF")
        .into();
    // The first 391 SIDs name CFF's standard strings, which aren't in the
    // String INDEX.
    for sid in dropped.into_iter().filter(|&s| s >= 391) {
        subset
            .string_index
            .replace(usize::from(sid - 391), Vec::new());
    }
    let mut out = WriteBuffer::new();
    CFF::write(&mut out, &subset).expect("allsorts writes the CFF");
    out.into_inner()
}

/// `MATH` for the kept glyphs, renumbered by `new_id`. Panics if a kept
/// glyph's variant or part isn't kept: the closure keeps them all.
fn subset_math(math: &Math, new_id: &dyn Fn(GlyphId) -> Option<GlyphId16>) -> Vec<u8> {
    let mut math: owned::Math = math.to_owned_table();
    let part = |g: GlyphId16| new_id(g.into()).expect("the closure keeps every part");
    let info = &mut *math.math_glyph_info;
    if let Some(t) = info.math_italics_correction_info.as_mut() {
        (*t.coverage, t.italics_correction) = keep(&t.coverage, &t.italics_correction, new_id);
    }
    if let Some(t) = info.math_top_accent_attachment.as_mut() {
        (*t.top_accent_coverage, t.top_accent_attachment) =
            keep(&t.top_accent_coverage, &t.top_accent_attachment, new_id);
    }
    if let Some(t) = info.extended_shape_coverage.as_mut() {
        *t = t.iter().filter_map(new_id).collect();
    }
    if let Some(t) = info.math_kern_info.as_mut() {
        (*t.math_kern_coverage, t.math_kern_info_records) =
            keep(&t.math_kern_coverage, &t.math_kern_info_records, new_id);
    }
    let v = &mut *math.math_variants;
    for (coverage, constructions) in [
        (&mut v.vert_glyph_coverage, &mut v.vert_glyph_constructions),
        (
            &mut v.horiz_glyph_coverage,
            &mut v.horiz_glyph_constructions,
        ),
    ] {
        let Some(c) = coverage.as_mut() else {
            continue;
        };
        let (new, mut kept) = keep(c, constructions, new_id);
        for construction in &mut kept {
            for v in &mut construction.math_glyph_variant_records {
                v.variant_glyph = part(v.variant_glyph);
            }
            if let Some(assembly) = construction.glyph_assembly.as_mut() {
                for p in &mut assembly.part_records {
                    p.glyph_id = part(p.glyph_id);
                }
            }
        }
        (*c, *constructions) = (new, kept);
    }
    write_fonts::dump_table(&math).expect("MATH writes")
}

/// The records of `coverage`'s glyphs that are kept, and their coverage
/// renumbered.
fn keep<T: Clone>(
    coverage: &CoverageTable,
    records: &[T],
    new_id: &dyn Fn(GlyphId) -> Option<GlyphId16>,
) -> (CoverageTable, Vec<T>) {
    let (glyphs, records): (Vec<GlyphId16>, Vec<T>) = coverage
        .iter()
        .zip(records)
        .filter_map(|(g, r)| Some((new_id(g)?, r.clone())))
        .unzip();
    (glyphs.into_iter().collect(), records)
}
