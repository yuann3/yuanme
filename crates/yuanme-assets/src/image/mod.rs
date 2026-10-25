//! The image pipeline (#8, #28): decode once, apply EXIF orientation,
//! convert to sRGB, resize once per width with Lanczos3, and encode on the
//! shared rayon pool. Every output is byte-identical on every machine.

#[cfg(test)]
mod tests;

use std::fmt;
use std::io::Cursor;

use fast_image_resize::images::{Image, ImageRef};
use fast_image_resize::{PixelType, Resizer};
use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageReader};
use moxcms::{ColorProfile, Layout, TransformOptions};
use rayon::prelude::*;

/// A larger source is an error at the reference (#28).
const MAX_PIXELS: u64 = 100_000_000;

/// rav1e splits an image into one tile per thread unless told otherwise, so
/// its bytes would follow the core count. Four tiles cost 1.1% in size (#8).
const AVIF_TILES: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    Avif,
    Jpeg,
    Png,
    Webp,
}

impl Format {
    pub const ALL: [Format; 4] = [Format::Avif, Format::Jpeg, Format::Png, Format::Webp];

    pub fn ext(self) -> &'static str {
        match self {
            Format::Avif => "avif",
            Format::Jpeg => "jpg",
            Format::Png => "png",
            Format::Webp => "webp",
        }
    }
}

/// Each number is on its encoder's own scale (#28).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub avif_quality: u8,
    pub avif_speed: u8,
    pub jpeg_quality: u8,
    pub webp_quality: u8,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            avif_quality: 72,
            avif_speed: 6,
            jpeg_quality: 80,
            webp_quality: 80,
        }
    }
}

/// What a source's header says, without decoding its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Probe {
    /// As displayed, once EXIF orientation is applied.
    pub width: u32,
    pub height: u32,
    /// The pixel format can hold transparency, whether or not any pixel uses
    /// it: that is all a header tells.
    pub alpha: bool,
    /// An animated GIF, WebP or PNG, which the pipeline would encode as its
    /// first frame, so it is copied instead (#28).
    pub animated: bool,
}

impl Probe {
    /// The height of an output `width` wide, rounded to the nearest pixel.
    pub fn height_at(&self, width: u32) -> u32 {
        let (w, h) = (u64::from(self.width), u64::from(self.height));
        let height = (2 * h * u64::from(width) + w) / (2 * w);
        u32::try_from(height).unwrap_or(u32::MAX).max(1)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    TooLarge {
        width: u32,
        height: u32,
    },
    /// The source isn't an image the pipeline reads, or it is damaged.
    Decode(String),
    Encode(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::TooLarge { width, height } => {
                write!(f, "{width}×{height} is over the 100-megapixel limit")
            }
            Error::Decode(e) => write!(f, "can't decode the image: {e}"),
            Error::Encode(e) => write!(f, "can't encode the image as {e}"),
        }
    }
}

impl std::error::Error for Error {}

fn decode_error(e: impl fmt::Display) -> Error {
    Error::Decode(e.to_string())
}

/// Reads only the header, so a Page can name and size every output of an
/// image before any is encoded.
pub fn probe(source: &[u8]) -> Result<Probe, Error> {
    let mut decoder = decoder(source)?;
    let animated = animated(source)?;
    let (width, height) = decoder.dimensions();
    let alpha = decoder.color_type().has_alpha();
    let turned = matches!(
        decoder.orientation().map_err(decode_error)?,
        Orientation::Rotate90
            | Orientation::Rotate270
            | Orientation::Rotate90FlipH
            | Orientation::Rotate270FlipH
    );
    let (width, height) = if turned {
        (height, width)
    } else {
        (width, height)
    };
    Ok(Probe {
        width,
        height,
        alpha,
        animated,
    })
}

fn animated(source: &[u8]) -> Result<bool, Error> {
    use image::codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder};
    use image::{AnimationDecoder, ImageFormat};
    let format = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(decode_error)?
        .format();
    Ok(match format {
        // A GIF says nothing of its frames up front: the second one has to
        // be found.
        Some(ImageFormat::Gif) => {
            let gif = GifDecoder::new(Cursor::new(source)).map_err(decode_error)?;
            gif.into_frames().take(2).count() > 1
        }
        Some(ImageFormat::WebP) => WebPDecoder::new(Cursor::new(source))
            .map_err(decode_error)?
            .has_animation(),
        Some(ImageFormat::Png) => PngDecoder::new(Cursor::new(source))
            .and_then(|png| png.is_apng())
            .map_err(decode_error)?,
        _ => false,
    })
}

/// Encodes `source` once for each `(width, format)`, in that order. Each
/// width is resized once, whatever the number of formats.
pub fn encode(
    source: &[u8],
    outputs: &[(u32, Format)],
    settings: &Settings,
) -> Result<Vec<Vec<u8>>, Error> {
    let pixels = decode(source)?;
    let mut widths: Vec<u32> = outputs.iter().map(|&(w, _)| w).collect();
    widths.sort_unstable();
    widths.dedup();
    let resized: Vec<Pixels> = widths.par_iter().map(|&w| pixels.resize(w)).collect();
    outputs
        .par_iter()
        .map(|&(width, format)| {
            let at = widths
                .binary_search(&width)
                .expect("every width is resized");
            resized[at].encode(format, settings)
        })
        .collect()
}

fn decoder(source: &[u8]) -> Result<impl ImageDecoder + '_, Error> {
    let decoder = ImageReader::new(Cursor::new(source))
        .with_guessed_format()
        .map_err(decode_error)?
        .into_decoder()
        .map_err(decode_error)?;
    let (width, height) = decoder.dimensions();
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(Error::TooLarge { width, height });
    }
    Ok(decoder)
}

/// 8-bit sRGB pixels as displayed, with alpha or without.
struct Pixels {
    width: u32,
    height: u32,
    alpha: bool,
    bytes: Vec<u8>,
}

fn decode(source: &[u8]) -> Result<Pixels, Error> {
    let mut decoder = decoder(source)?;
    let orientation = decoder.orientation().map_err(decode_error)?;
    let icc = decoder.icc_profile().map_err(decode_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    image.apply_orientation(orientation);
    // Nothing else the source carries is read, so no output can carry its
    // EXIF, XMP or IPTC, such as where a phone photo was taken (#28).
    let (width, height) = (image.width(), image.height());
    let alpha = image.color().has_alpha();
    let mut bytes = if alpha {
        image.into_rgba8().into_raw()
    } else {
        image.into_rgb8().into_raw()
    };
    if let Some(icc) = icc {
        to_srgb(&icc, alpha, &mut bytes);
    }
    Ok(Pixels {
        width,
        height,
        alpha,
        bytes,
    })
}

/// Converts from the source's ICC profile to sRGB, as sharp does, so a
/// Display P3 photo looks the same in every browser and format (#28). A
/// profile that can't apply to RGB, or can't be read, is ignored, as
/// browsers ignore it.
fn to_srgb(icc: &[u8], alpha: bool, bytes: &mut [u8]) {
    let layout = if alpha { Layout::Rgba } else { Layout::Rgb };
    let srgb = ColorProfile::new_srgb();
    let Some(transform) = ColorProfile::new_from_slice(icc).ok().and_then(|profile| {
        profile
            .create_transform_8bit(layout, &srgb, layout, TransformOptions::default())
            .ok()
    }) else {
        return;
    };
    let source = bytes.to_vec();
    transform
        .transform(&source, bytes)
        .expect("source and destination have the transform's layout and length");
}

impl Pixels {
    fn probe(&self) -> Probe {
        Probe {
            width: self.width,
            height: self.height,
            alpha: self.alpha,
            animated: false,
        }
    }

    fn pixel_type(&self) -> PixelType {
        if self.alpha {
            PixelType::U8x4
        } else {
            PixelType::U8x3
        }
    }

    fn resize(&self, width: u32) -> Pixels {
        let height = self.probe().height_at(width);
        let source = ImageRef::new(self.width, self.height, &self.bytes, self.pixel_type())
            .expect("the buffer holds width × height pixels");
        let mut resized = Image::new(width, height, self.pixel_type());
        // Lanczos3 on the sRGB values, as sharp does. Its default options
        // premultiply alpha.
        Resizer::new()
            .resize(&source, &mut resized, None)
            .expect("both images have the same pixel type");
        Pixels {
            width,
            height,
            alpha: self.alpha,
            bytes: resized.into_vec(),
        }
    }

    fn encode(&self, format: Format, settings: &Settings) -> Result<Vec<u8>, Error> {
        match format {
            Format::Avif => self.avif(settings),
            Format::Jpeg => self.jpeg(settings),
            Format::Png => self.png(),
            Format::Webp => self.webp(settings),
        }
    }

    fn avif(&self, settings: &Settings) -> Result<Vec<u8>, Error> {
        let encoder = ravif::Encoder::new()
            .with_quality(f32::from(settings.avif_quality))
            .with_speed(settings.avif_speed)
            .with_bit_depth(ravif::BitDepth::Ten)
            .with_tiles(Some(AVIF_TILES));
        let (width, height) = (self.width as usize, self.height as usize);
        let encoded = if self.alpha {
            let pixels: Vec<_> = self
                .bytes
                .as_chunks::<4>()
                .0
                .iter()
                .map(|&[r, g, b, a]| ravif::RGBA8::new(r, g, b, a))
                .collect();
            encoder.encode_rgba(ravif::Img::new(&pixels[..], width, height))
        } else {
            let pixels: Vec<_> = self
                .bytes
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| ravif::RGB8::new(r, g, b))
                .collect();
            encoder.encode_rgb(ravif::Img::new(&pixels[..], width, height))
        };
        encoded
            .map(|e| e.avif_file)
            .map_err(|e| Error::Encode(format!("AVIF: {e}")))
    }

    /// libvips' settings, which sharp uses: baseline, Annex K tables,
    /// optimised Huffman tables and 4:2:0 (#8).
    fn jpeg(&self, settings: &Settings) -> Result<Vec<u8>, Error> {
        let rgb = self.rgb();
        mozjpeg_rs::Encoder::fastest()
            .quality(settings.jpeg_quality)
            .quant_tables(mozjpeg_rs::QuantTableIdx::JpegAnnexK)
            .optimize_huffman(true)
            .encode_rgb(&rgb, self.width, self.height)
            .map_err(|e| Error::Encode(format!("JPEG: {e}")))
    }

    fn png(&self) -> Result<Vec<u8>, Error> {
        use image::codecs::png::{CompressionType, FilterType, PngEncoder};
        use image::{ExtendedColorType, ImageEncoder};
        let color = if self.alpha {
            ExtendedColorType::Rgba8
        } else {
            ExtendedColorType::Rgb8
        };
        let mut out = Vec::new();
        PngEncoder::new_with_quality(&mut out, CompressionType::Default, FilterType::Adaptive)
            .write_image(&self.bytes, self.width, self.height, color)
            .map_err(|e| Error::Encode(format!("PNG: {e}")))?;
        Ok(out)
    }

    /// libwebp's simple API is method 4, sharp's default (#8).
    fn webp(&self, settings: &Settings) -> Result<Vec<u8>, Error> {
        let (width, height) = (self.width as i32, self.height as i32);
        let quality = f32::from(settings.webp_quality);
        let mut out = std::ptr::null_mut();
        // SAFETY: `bytes` holds `height` rows of `width` pixels, each 3 or 4
        // bytes as the function expects, and libwebp only reads it. The
        // returned buffer is copied and then freed with WebPFree.
        unsafe {
            let len = if self.alpha {
                libwebp_sys::WebPEncodeRGBA(
                    self.bytes.as_ptr(),
                    width,
                    height,
                    width * 4,
                    quality,
                    &mut out,
                )
            } else {
                libwebp_sys::WebPEncodeRGB(
                    self.bytes.as_ptr(),
                    width,
                    height,
                    width * 3,
                    quality,
                    &mut out,
                )
            };
            if len == 0 {
                return Err(Error::Encode(format!(
                    "WebP: libwebp takes at most 16383×16383, not {}×{}",
                    self.width, self.height
                )));
            }
            let bytes = std::slice::from_raw_parts(out, len).to_vec();
            libwebp_sys::WebPFree(out.cast());
            Ok(bytes)
        }
    }

    /// JPEG has no alpha, so it is dropped.
    fn rgb(&self) -> Vec<u8> {
        if self.alpha {
            self.bytes
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|&[r, g, b, _]| [r, g, b])
                .collect()
        } else {
            self.bytes.clone()
        }
    }
}
