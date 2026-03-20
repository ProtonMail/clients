//! Benchmark comparing the two HTML-to-text extraction mechanisms.
//!
//! 1. **Transformer + html2text**: Full DOM parse (kuchikiki/html5ever) + html2text render.
//!    Used for display; was previously used for search indexing.
//! 2. **`html_to_text_fast`**: Single-pass tag stripper, no DOM. Used for search indexing.
//!

use criterion::{Criterion, criterion_group, criterion_main};
use mail_html_transformer::{Html2TextOptions, Transformer, html_to_text_fast};
use std::hint::black_box;

static AMOS_HTTP: &str = include_str!("./amos_http.html");
static AMOS_LANDING: &str = include_str!("./amos_landing.html");
static IMGS: &str = include_str!("./100_imgs.html");
static LINKS: &str = include_str!("./100_links.html");
static PATHOLOGICAL_BLOCKQUOTES: &str = include_str!("./pathological_blockquotes.html");

const FIXTURES: &[(&str, &str)] = &[
    ("amos_http", AMOS_HTTP),
    ("amos_landing", AMOS_LANDING),
    ("100_imgs", IMGS),
    ("100_links", LINKS),
    ("pathological_blockquotes", PATHOLOGICAL_BLOCKQUOTES),
];

fn transformer_to_plain_text(html: &str) -> String {
    let tr = Transformer::new(html);
    tr.to_plain_text(Html2TextOptions::default())
        .unwrap_or_default()
}

fn bench_html_extract_compare(c: &mut Criterion) {
    let mut group = c.benchmark_group("html_extract_compare");
    group.sample_size(50);

    for (name, html) in FIXTURES {
        let html_len = html.len();
        group.throughput(criterion::Throughput::Bytes(html_len as u64));

        group.bench_with_input(
            criterion::BenchmarkId::new("transformer_html2text", name),
            html,
            |b, input| {
                b.iter(|| {
                    let _ = black_box(transformer_to_plain_text(black_box(input)));
                });
            },
        );

        group.bench_with_input(
            criterion::BenchmarkId::new("html_to_text_fast", name),
            html,
            |b, input| {
                b.iter(|| {
                    let _ = black_box(html_to_text_fast(black_box(input)));
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_html_extract_compare);
criterion_main!(benches);
