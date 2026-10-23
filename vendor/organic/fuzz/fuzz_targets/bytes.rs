#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    if let Ok(text) = std::str::from_utf8(data) {
        yuanme_org_fuzz::check(text);
    }
});
