use super::*;
use crate::Body;

const FIXTURES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../yuanme-assets/fixtures/images"
);

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{FIXTURES}/{name}")).unwrap()
}

const WIDTHS: [u32; 5] = [480, 800, 1200, 1600, 2400];

fn sizes(variants: &[Variant]) -> Vec<(u32, u32, Format)> {
    variants
        .iter()
        .map(|v| (v.width, v.height, v.format))
        .collect()
}

fn one(mut outputs: Vec<Result<Output, Error>>) -> Result<Output, Error> {
    assert_eq!(outputs.len(), 1);
    outputs.pop().unwrap()
}

fn decoded_width(body: &Body) -> u32 {
    let Body::Bytes(bytes) = body else {
        panic!("a memory cache holds bytes, not {body:?}")
    };
    image::load_from_memory(bytes).unwrap().width()
}

#[test]
fn variants_are_named_and_sized_before_any_is_encoded() {
    let mut cache = Cache::memory("1.0.0");
    let mut inputs = Inputs::default();
    // 1080×1440 as displayed: narrower than the widest width asked for.
    inputs.insert("content/posts/sea/park.jpg", fixture("park.jpg"));
    let settings = Settings::default();

    let probe = probe(&mut cache, &inputs, "content/posts/sea/park.jpg").unwrap();
    let formats = [Format::Avif, Format::Jpeg];
    let park = variants(
        &cache,
        &inputs,
        "content/posts/sea/park.jpg",
        probe,
        &WIDTHS,
        &formats,
        &settings,
    );

    // Never upscaled: the source's own width is the last.
    let both = |w, h| [(w, h, Format::Avif), (w, h, Format::Jpeg)];
    let expected = [both(480, 640), both(800, 1067), both(1080, 1440)].concat();
    assert_eq!(sizes(&park), expected);
    let jpeg_480 = &park[1];
    assert_eq!(
        jpeg_480.path,
        format!("assets/park-480-{}.jpg", &jpeg_480.key.to_hex()[..16])
    );
    assert!(park.iter().all(|v| cache.get_large(&v.key).is_none()));

    // A source wider than every width gets exactly those widths.
    inputs.insert("IMG 0001.JPG", fixture("beach.jpg"));
    let probe = super::probe(&mut cache, &inputs, "IMG 0001.JPG").unwrap();
    let beach = variants(
        &cache,
        &inputs,
        "IMG 0001.JPG",
        probe,
        &[800, 480],
        &[Format::Avif],
        &settings,
    );
    assert_eq!(
        sizes(&beach),
        [(480, 320, Format::Avif), (800, 533, Format::Avif)]
    );
    // A srcset splits on whitespace.
    assert!(beach[0].path.starts_with("assets/IMG-0001-480-"));
}

#[test]
fn a_name_changes_with_everything_its_bytes_depend_on_and_nothing_else() {
    let settings = Settings::default();
    let names = |version: &str, source: &[u8], settings: &Settings| {
        let mut cache = Cache::memory(version);
        let mut inputs = Inputs::default();
        inputs.insert("a.jpg", source);
        let probe = probe(&mut cache, &inputs, "a.jpg").unwrap();
        let formats = [Format::Avif, Format::Jpeg];
        variants(&cache, &inputs, "a.jpg", probe, &[480], &formats, settings)
            .into_iter()
            .map(|v| v.path)
            .collect::<Vec<_>>()
    };
    let beach = fixture("beach.jpg");
    let [avif, jpeg] = names("1.0.0", &beach, &settings).try_into().unwrap();

    let sharper = Settings {
        jpeg_quality: 90,
        ..settings.clone()
    };
    let [same_avif, new_jpeg] = names("1.0.0", &beach, &sharper).try_into().unwrap();
    assert_eq!(same_avif, avif, "the JPEG quality renamed the AVIF");
    assert_ne!(new_jpeg, jpeg);
    let slower = Settings {
        avif_speed: 4,
        ..settings.clone()
    };
    let [new_avif, same_jpeg] = names("1.0.0", &beach, &slower).try_into().unwrap();
    assert_ne!(new_avif, avif);
    assert_eq!(same_jpeg, jpeg, "the AVIF speed renamed the JPEG");

    // Same size, other pixels.
    let mut other = beach.clone();
    *other.last_mut().unwrap() ^= 1;
    let renamed = |names: Vec<String>| names[0] != avif && names[1] != jpeg;
    assert!(renamed(names("1.0.0", &other, &settings)));
    assert!(renamed(names("1.0.1", &beach, &settings)));
}

#[test]
fn encode_fills_the_cache_once_and_dev_can_ask_for_one_variant() {
    let mut cache = Cache::memory("1.0.0");
    let mut inputs = Inputs::default();
    inputs.insert("shaokao.webp", fixture("shaokao.webp"));
    let settings = Settings::default();
    let probe = probe(&mut cache, &inputs, "shaokao.webp").unwrap();
    let all = variants(
        &cache,
        &inputs,
        "shaokao.webp",
        probe,
        &[160, 320],
        &[Format::Avif, Format::Png],
        &settings,
    );

    // `yuanme dev` encodes what a browser asks for, and only that.
    let png_320 = one(encode(&mut cache, &inputs, &all[3..], &settings)).unwrap();
    assert_eq!(decoded_width(&png_320.body), 320);
    // Its key stands in for the hash of its bytes, which it determines.
    assert_eq!(png_320.hash, all[3].key);
    assert!(cache.get_large(&all[0].key).is_none());

    let outputs = encode(&mut cache, &inputs, &all, &settings);
    assert_eq!(outputs.len(), 4);
    assert_eq!(decoded_width(&outputs[1].as_ref().unwrap().body), 160);
    assert!(all.iter().all(|v| cache.get_large(&v.key).is_some()));

    // A warm build reads neither the header nor the pixels again.
    let mut cold = Inputs::default();
    cold.insert_streamed("shaokao.webp", inputs.files["shaokao.webp"].hash);
    assert_eq!(
        super::probe(&mut cache, &cold, "shaokao.webp").unwrap(),
        probe
    );
    let again = encode(&mut cache, &cold, &all, &settings);
    assert!(
        again
            .iter()
            .zip(&outputs)
            .all(|(a, o)| a.as_ref().unwrap().body == o.as_ref().unwrap().body)
    );
}

#[test]
fn a_streamed_source_is_read_from_the_project_and_checked() {
    let dir = tempfile::tempdir().unwrap();
    let park = fixture("park.jpg");
    std::fs::write(dir.path().join("park.jpg"), &park).unwrap();
    let mut inputs = Inputs::at(dir.path().to_path_buf());
    inputs.insert_streamed("park.jpg", blake3::hash(&park));
    let mut cache = Cache::memory("1.0.0");
    let settings = Settings::default();

    let probe = probe(&mut cache, &inputs, "park.jpg").unwrap();
    assert_eq!((probe.width, probe.height), (1080, 1440));
    let [jpeg] = variants(
        &cache,
        &inputs,
        "park.jpg",
        probe,
        &[160],
        &[Format::Jpeg],
        &settings,
    )
    .try_into()
    .unwrap();

    // Saved again after the build loaded it: encoding the new bytes under
    // the old hash's name would serve them from a stale cache entry forever.
    std::fs::write(dir.path().join("park.jpg"), fixture("beach.jpg")).unwrap();
    let changed = one(encode(
        &mut cache,
        &inputs,
        std::slice::from_ref(&jpeg),
        &settings,
    ));
    assert_eq!(changed.err(), Some(Error::Changed));
    assert!(cache.get_large(&jpeg.key).is_none());

    std::fs::write(dir.path().join("park.jpg"), &park).unwrap();
    let encoded = one(encode(&mut cache, &inputs, &[jpeg], &settings)).unwrap();
    assert_eq!(decoded_width(&encoded.body), 160);
}
