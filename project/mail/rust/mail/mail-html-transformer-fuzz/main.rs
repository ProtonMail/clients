#![no_main]

#[macro_use]
extern crate libfuzzer_sys;

fuzz_target!(|data: &str| {
    _ = mail_html_transformer::Transformer::new(data);
    _ = mail_html_transformer::html_to_text_fast(data);
});
