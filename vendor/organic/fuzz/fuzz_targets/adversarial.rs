#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    yuanme_org_fuzz::check(&yuanme_org_fuzz::adversarial(data));
});
