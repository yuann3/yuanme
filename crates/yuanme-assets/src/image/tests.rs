use image::ImageEncoder;
use image::codecs::png::PngEncoder;

use super::*;

const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/images");

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap()
}

fn on_threads<T: Send>(threads: usize, f: impl FnOnce() -> T + Send) -> T {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .unwrap()
        .install(f)
}

/// The size an encoded image decodes to.
fn decoded_size(bytes: &[u8]) -> (u32, u32) {
    // `image` decodes AVIF only through C dav1d, so read its `ispe` box:
    // version and flags, then width and height.
    if let Some(at) = bytes.windows(4).position(|w| w == b"ispe") {
        let be = |i: usize| u32::from_be_bytes(bytes[at + i..at + i + 4].try_into().unwrap());
        return (be(8), be(12));
    }
    image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .unwrap()
        .into_dimensions()
        .unwrap()
}

/// Outputs are byte-identical across machines, core counts and CPU
/// architectures (#28), so CI and laptops can share `.yuanme/cache`. CI runs
/// this on Linux x86_64 and macOS arm64 against the one snapshot.
#[test]
fn every_machine_encodes_the_same_bytes() {
    let widths = [160, 240, 320, 480, 640];
    let mut lines = Vec::new();
    for name in ["beach.jpg", "fallingwater.jpg", "park.jpg", "shaokao.webp"] {
        let source = fixture(name);
        let outputs: Vec<_> = widths
            .iter()
            .flat_map(|&w| Format::ALL.map(|f| (w, f)))
            .collect();
        let settings = Settings::default();
        let encoded = on_threads(1, || encode(&source, &outputs, &settings).unwrap());
        assert!(
            encoded == on_threads(3, || encode(&source, &outputs, &settings).unwrap()),
            "{name}: the bytes depend on the number of threads"
        );
        for (&(width, format), bytes) in outputs.iter().zip(&encoded) {
            // Phone photos carry where they were taken: park.jpg has GPS
            // EXIF and an ICC profile, and none of it may survive.
            for marker in [
                "Exif",
                "EXIF",
                "eXIf",
                "ICC_PROFILE",
                "iCCP",
                "ICCP",
                "xmpmeta",
            ] {
                assert!(
                    !bytes.windows(marker.len()).any(|w| w == marker.as_bytes()),
                    "{name} at {width}w as {format:?} keeps {marker}"
                );
            }
            let hash = blake3::hash(bytes).to_hex();
            let ext = format.ext();
            lines.push(format!(
                "{name} {width}w {ext}: {} {}",
                bytes.len(),
                &hash[..16]
            ));
        }
    }
    insta::assert_snapshot!(lines.join("\n"));
}

#[test]
fn probe_gives_the_size_as_displayed_and_outputs_keep_its_shape() {
    // Stored 1440×1080 with EXIF orientation 6, as a phone does.
    let park = probe(&fixture("park.jpg")).unwrap();
    assert_eq!((park.width, park.height, park.alpha), (1080, 1440, false));
    let shaokao = probe(&fixture("shaokao.webp")).unwrap();
    assert_eq!(
        (shaokao.width, shaokao.height, shaokao.alpha),
        (1200, 900, true)
    );
    // 1600×1066 at 480 wide is 319.8 high: rounded, not truncated.
    let beach = probe(&fixture("beach.jpg")).unwrap();
    assert_eq!(beach.height_at(480), 320);

    let outputs = [(480, Format::Avif), (480, Format::Jpeg)];
    let settings = Settings::default();
    for (source, probe) in [("park.jpg", park), ("beach.jpg", beach)] {
        for bytes in encode(&fixture(source), &outputs, &settings).unwrap() {
            assert_eq!(
                decoded_size(&bytes),
                (480, probe.height_at(480)),
                "{source}"
            );
        }
    }
}

#[test]
fn a_display_p3_source_is_converted_to_srgb() {
    let mut source = Vec::new();
    let mut png = PngEncoder::new(&mut source);
    png.set_icc_profile(moxcms::ColorProfile::new_display_p3().encode().unwrap())
        .unwrap();
    png.write_image(
        &[200, 80, 40].repeat(64),
        8,
        8,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();

    let [png] = encode(&source, &[(8, Format::Png)], &Settings::default())
        .unwrap()
        .try_into()
        .unwrap();

    let pixel = image::load_from_memory(&png).unwrap().to_rgb8()[(4, 4)].0;
    // P3 (200, 80, 40) is sRGB (216.4, 68.9, 17.1), through XYZ with the
    // D65 matrices of both spaces.
    let srgb = [216, 69, 17];
    assert!(
        pixel.iter().zip(srgb).all(|(p, s)| p.abs_diff(s) <= 2),
        "{pixel:?} is not {srgb:?}"
    );
}

/// A PNG that is only a header: enough to probe, too little to decode.
fn png_header(width: u32, height: u32) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut crc = !0u32;
        for &byte in &out[start..] {
            crc ^= u32::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 {
                    0xEDB8_8320 ^ (crc >> 1)
                } else {
                    crc >> 1
                };
            }
        }
        out.extend_from_slice(&(!crc).to_be_bytes());
    }
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let ihdr = [
        &width.to_be_bytes()[..],
        &height.to_be_bytes(),
        &[8, 2, 0, 0, 0],
    ]
    .concat();
    chunk(&mut png, b"IHDR", &ihdr);
    // An empty zlib stream.
    chunk(
        &mut png,
        b"IDAT",
        &[0x78, 0x9c, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01],
    );
    chunk(&mut png, b"IEND", &[]);
    png
}

#[test]
fn a_source_over_100_megapixels_is_refused_before_it_is_decoded() {
    let too_large = png_header(10_001, 10_000);
    let refused = Error::TooLarge {
        width: 10_001,
        height: 10_000,
    };
    assert_eq!(probe(&too_large).unwrap_err(), refused);
    // Decoding would fail too, but as a decode error, after allocating 300 MB.
    let outputs = [(160, Format::Jpeg)];
    assert_eq!(
        encode(&too_large, &outputs, &Settings::default()).unwrap_err(),
        refused
    );

    assert!(probe(&png_header(10_000, 10_000)).is_ok());
    assert!(matches!(probe(b"GIF89a"), Err(Error::Decode(_))));
}

/// #15's Gate on `ubuntu-latest`, measured as #15 measured it: four photos
/// at 160–640 px in AVIF, WebP and JPEG, all on one pool, as a median of
/// five runs.
#[test]
#[ignore = "a Gate, timed in a release build on CI"]
fn encoding_takes_at_most_75_ms_of_wall_time_per_output() {
    let sources = ["beach.jpg", "fallingwater.jpg", "park.jpg", "shaokao.webp"].map(fixture);
    let outputs: Vec<_> = [160, 240, 320, 480, 640]
        .into_iter()
        .flat_map(|w| [Format::Avif, Format::Webp, Format::Jpeg].map(|f| (w, f)))
        .collect();
    let settings = Settings::default();
    let run = || {
        let start = std::time::Instant::now();
        sources.par_iter().for_each(|source| {
            encode(source, &outputs, &settings).unwrap();
        });
        start.elapsed()
    };
    run();
    let mut runs: Vec<_> = (0..5).map(|_| run()).collect();
    runs.sort();
    let count = (sources.len() * outputs.len()) as u32;
    let per_output = runs[2] / count;
    eprintln!(
        "{count} outputs in {:?} on {} threads: {per_output:?} each",
        runs[2],
        rayon::current_num_threads()
    );
    assert!(per_output <= std::time::Duration::from_millis(75));
}
