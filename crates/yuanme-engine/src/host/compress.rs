//! Brotli and gzip copies of text files, which the static adapter writes
//! with `precompress` and the Site budgets measure.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::Arc;

use rayon::prelude::*;

use crate::{Body, Cache, Output};

/// Text that Caddy's `precompressed` and nginx's `brotli_static` and
/// `gzip_static` serve from a sibling file.
const TEXT: &[&str] = &[
    ".html",
    ".css",
    ".js",
    ".mjs",
    ".json",
    ".xml",
    ".txt",
    ".svg",
    ".webmanifest",
];

/// A `.br` at quality 11 and a `.gz` beside each text file.
pub(super) fn precompressed(
    files: &BTreeMap<String, Output>,
    cache: &mut Cache,
) -> Vec<(String, Vec<u8>)> {
    let text: Vec<(&String, &[u8])> = files
        .iter()
        .filter(|(path, _)| TEXT.iter().any(|e| path.ends_with(e)))
        .filter_map(|(path, output)| match &output.body {
            Body::Bytes(bytes) => Some((path, &bytes[..])),
            _ => None,
        })
        .collect();
    let bytes: Vec<&[u8]> = text.iter().map(|(_, bytes)| *bytes).collect();
    let br = brotli(&bytes, 11, cache);
    let gz: Vec<Vec<u8>> = bytes.par_iter().map(|bytes| gzip(bytes)).collect();
    let mut out = Vec::new();
    for (((path, _), br), gz) in text.iter().zip(br).zip(gz) {
        out.push((format!("{path}.br"), br.to_vec()));
        out.push((format!("{path}.gz"), gz));
    }
    out
}

/// Each of `texts` at Brotli `quality`, cached by the input's bytes
/// (ADR 0011): quality 11 takes about a second per MB. The Site budgets
/// (#15) share the cache with the `.br` files.
pub(crate) fn brotli(texts: &[&[u8]], quality: i32, cache: &mut Cache) -> Vec<Arc<[u8]>> {
    let keys: Vec<blake3::Hash> = texts
        .iter()
        .map(|bytes| {
            let mut key = cache.key("brotli");
            key.update(&quality.to_le_bytes());
            key.update(bytes);
            key.finalize()
        })
        .collect();
    let cached: Vec<Option<Arc<[u8]>>> = keys.iter().map(|k| cache.get("brotli", k)).collect();
    let made: Vec<(Arc<[u8]>, bool)> = texts
        .par_iter()
        .zip(cached)
        .map(|(bytes, cached)| match cached {
            Some(br) => (br, false),
            None => (compress(bytes, quality).into(), true),
        })
        .collect();
    let mut out = Vec::with_capacity(made.len());
    for (key, (br, fresh)) in keys.into_iter().zip(made) {
        if fresh {
            cache.insert("brotli", key, br.clone());
        }
        out.push(br);
    }
    out
}

fn compress(bytes: &[u8], quality: i32) -> Vec<u8> {
    let params = brotli::enc::BrotliEncoderParams {
        quality,
        lgwin: 22,
        ..Default::default()
    };
    let mut out = Vec::new();
    brotli::BrotliCompress(&mut &bytes[..], &mut out, &params)
        .expect("writing to a Vec can't fail");
    out
}

/// No file name and a zero time in its header, so the bytes are the same
/// every build.
fn gzip(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    encoder
        .write_all(bytes)
        .expect("writing to a Vec can't fail");
    encoder.finish().expect("writing to a Vec can't fail")
}
