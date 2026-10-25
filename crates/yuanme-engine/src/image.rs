//! Image variants (#62). Each is named by its cache key, so a Page can
//! refer to it before it is encoded, and `yuanme dev` can encode it only
//! when a browser asks. Encoded variants are large cache entries
//! (ADR 0011), so a warm build reads no image at all.

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Mutex;

use rayon::prelude::*;
use yuanme_assets::image as pipeline;
pub use yuanme_assets::image::{Format, Probe, Settings};

use crate::link::hashed_path;
use crate::{Cache, Inputs, Output};

/// One output of one source image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Variant {
    /// By its path in [`Inputs`].
    pub source: String,
    pub width: u32,
    pub height: u32,
    pub format: Format,
    /// Covers everything its bytes depend on: the Engine version, which
    /// pins the encoders exactly (#27), the source's hash, its width and
    /// format, and that format's settings.
    pub key: blake3::Hash,
    /// In the output directory, such as `assets/park-480-0123456789abcdef.avif`.
    pub path: String,
}

/// Why a source image has no outputs: an error at the reference (#28).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Read(String),
    /// The file is no longer what the build loaded, so its outputs would be
    /// cached under the old bytes' names.
    Changed,
    Image(pipeline::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Read(e) => write!(f, "can't read the image: {e}"),
            Error::Changed => f.write_str("the image changed during the build"),
            Error::Image(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for Error {}

/// The size of `source`, a file in `inputs`, as displayed, and whether it
/// is animated: from the cache, or else from its header.
pub fn probe(cache: &mut Cache, inputs: &Inputs, source: &str) -> Result<Probe, Error> {
    let mut key = cache.key("image-probe");
    key.update(inputs.files[source].hash.as_bytes());
    let key = key.finalize();
    if let Some(bytes) = cache.get("image-probe", &key) {
        let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().expect("4 bytes"));
        return Ok(Probe {
            width: u32_at(0),
            height: u32_at(4),
            alpha: bytes[8] == 1,
            animated: bytes[9] == 1,
        });
    }
    let probe = pipeline::probe(&read(inputs, source)?).map_err(Error::Image)?;
    let bytes = [
        &probe.width.to_le_bytes()[..],
        &probe.height.to_le_bytes(),
        &[u8::from(probe.alpha), u8::from(probe.animated)],
    ]
    .concat();
    cache.insert("image-probe", key, bytes);
    Ok(probe)
}

/// The variants of `source` at each of `widths` in each of `formats`,
/// narrowest first, without encoding any. Nothing is upscaled: a source
/// narrower than a width gets its own width instead (#28).
pub fn variants(
    cache: &Cache,
    inputs: &Inputs,
    source: &str,
    probe: Probe,
    widths: &[u32],
    formats: &[Format],
    settings: &Settings,
) -> Vec<Variant> {
    let mut widths: Vec<u32> = widths.iter().map(|&w| w.min(probe.width)).collect();
    widths.sort_unstable();
    widths.dedup();
    let hash = inputs.files[source].hash;
    let name = source.rsplit('/').next().unwrap_or(source);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
    // A srcset splits on whitespace, and a URL would need much else escaped.
    let stem: String = stem
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => c,
            _ => '-',
        })
        .collect();
    let mut variants = Vec::new();
    for &width in &widths {
        for &format in formats {
            let mut key = cache.key("image");
            key.update(hash.as_bytes());
            key.update(&width.to_le_bytes());
            key.update(format.ext().as_bytes());
            // Only the format's own settings, so a new JPEG quality doesn't
            // rename every AVIF.
            match format {
                Format::Avif => key.update(&[settings.avif_quality, settings.avif_speed]),
                Format::Jpeg => key.update(&[settings.jpeg_quality]),
                Format::Webp => key.update(&[settings.webp_quality]),
                Format::Png => &mut key,
            };
            let key = key.finalize();
            let path = hashed_path(&format!("{stem}-{width}.{}", format.ext()), &key);
            variants.push(Variant {
                source: source.to_string(),
                width,
                height: probe.height_at(width),
                format,
                key,
                path,
            });
        }
    }
    variants
}

/// Where each variant's bytes are, in order, encoding those the cache
/// lacks. Each source is read and decoded once, and sources are encoded in
/// parallel. A build passes every variant its Pages use; `yuanme dev`
/// passes the one a browser asked for.
pub fn encode(
    cache: &mut Cache,
    inputs: &Inputs,
    variants: &[Variant],
    settings: &Settings,
) -> Vec<Result<Output, Error>> {
    let mut misses: BTreeMap<&str, Vec<&Variant>> = BTreeMap::new();
    for variant in variants {
        if cache.get_large(&variant.key).is_none() {
            misses.entry(&variant.source).or_default().push(variant);
        }
    }
    let shared = Mutex::new(&mut *cache);
    let failed: BTreeMap<&str, Error> = misses
        .into_par_iter()
        .filter_map(|(source, variants)| {
            let outputs: Vec<_> = variants.iter().map(|v| (v.width, v.format)).collect();
            let encoded = read(inputs, source).and_then(|bytes| {
                pipeline::encode(&bytes, &outputs, settings).map_err(Error::Image)
            });
            // Inserted as each source finishes, so no more than one
            // source's outputs per thread are held in memory.
            let inserted = encoded.and_then(|encoded| {
                let mut cache = shared.lock().expect("inserting doesn't panic");
                variants.iter().zip(encoded).try_for_each(|(v, bytes)| {
                    cache
                        .insert_large(v.key, &bytes)
                        .map_err(|e| Error::Read(format!("writing the cache: {e}")))
                })
            });
            inserted.err().map(|e| (source, e))
        })
        .collect();
    variants
        .iter()
        .map(|variant| match cache.get_large(&variant.key) {
            Some(body) => Ok(Output {
                body,
                hash: variant.key,
            }),
            None => Err(failed[variant.source.as_str()].clone()),
        })
        .collect()
}

/// The source's bytes, which must be the ones the build loaded.
fn read(inputs: &Inputs, source: &str) -> Result<Vec<u8>, Error> {
    let input = &inputs.files[source];
    if let Some(bytes) = &input.bytes {
        return Ok(bytes.to_vec());
    }
    let root = inputs
        .root
        .as_ref()
        .expect("only a project's Inputs stream files");
    let bytes = std::fs::read(root.join(source)).map_err(|e| Error::Read(e.to_string()))?;
    if blake3::hash(&bytes) != input.hash {
        return Err(Error::Changed);
    }
    Ok(bytes)
}
