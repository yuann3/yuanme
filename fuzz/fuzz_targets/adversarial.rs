#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    yuanme_markdown_fuzz::check(&yuanme_markdown_fuzz::adversarial(data));
});
