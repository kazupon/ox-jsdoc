// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//
// Bench oxc's internal `oxc_jsdoc` crate (used by oxlint) against the same
// fixtures as `parser.rs`. Provides a third Rust-direct comparison axis
// alongside ox_jsdoc (typed AST) and ox_jsdoc_binary (binary AST).

use ox_jsdoc_benchmark::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxc_jsdoc::JSDoc;
use oxc_span::Span;

fn bench_oxc_jsdoc(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("oxc_jsdoc");
    let fixtures = ox_jsdoc_benchmark::load_perf_fixtures();

    for fixture in fixtures {
        let id = BenchmarkId::new(&fixture.bucket, &fixture.name);
        let comment_texts = &fixture.comment_texts;

        group.bench_function(id, |b| {
            b.iter(|| {
                let mut tag_count = 0usize;
                for source_text in comment_texts {
                    // oxc_jsdoc's `JSDoc::new` expects the inner content (no
                    // leading `/**`, no trailing `*/`). Mirror what oxlint's
                    // semantic builder feeds it: strip the delimiters off the
                    // raw source text and pass the start offset of the full
                    // comment as the span base.
                    let inner = source_text
                        .strip_prefix("/**")
                        .and_then(|s| s.strip_suffix("*/"))
                        .unwrap_or(black_box(source_text));
                    #[allow(clippy::cast_possible_truncation)]
                    let span = Span::new(0, source_text.len() as u32);
                    let jsdoc = JSDoc::new(inner, span);
                    // `tags()` triggers the lazy parse — without this the
                    // benchmark would only measure the constructor.
                    tag_count += jsdoc.tags().len();
                }
                black_box(tag_count);
            });
        });
    }

    group.finish();
}

criterion_group!(oxc_jsdoc, bench_oxc_jsdoc);
criterion_main!(oxc_jsdoc);
