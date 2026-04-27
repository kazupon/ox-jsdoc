// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use ox_jsdoc::{
    ParseOptions, SerializeOptions, ValidationOptions, analyze_comment, parse_comment,
    serialize_comment_json, serialize_comment_json_with_options, validate_comment,
};
use ox_jsdoc_benchmark::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxc_allocator::Allocator;

fn bench_serializer(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parser_validator_serializer");
    let fixtures = ox_jsdoc_benchmark::load_perf_fixtures();

    for fixture in fixtures {
        let id = BenchmarkId::new(&fixture.bucket, &fixture.name);
        let comment_texts = &fixture.comment_texts;

        group.bench_function(id, |b| {
            let mut allocator = Allocator::default();

            b.iter(|| {
                let mut parsed_count = 0usize;
                let mut diagnostic_count = 0usize;
                let mut json_len = 0usize;
                for source_text in comment_texts {
                    let parsed = parse_comment(
                        &allocator,
                        black_box(source_text),
                        0,
                        ParseOptions::default(),
                    );
                    if let Some(comment) = parsed.comment.as_ref() {
                        let validation = validate_comment(comment, ValidationOptions::default());
                        let analysis = analyze_comment(comment);
                        let json =
                            serialize_comment_json(comment, Some(&validation), Some(&analysis));
                        diagnostic_count += validation.diagnostics.len();
                        json_len += json.len();
                        parsed_count += 1;
                    }
                    diagnostic_count += parsed.diagnostics.len();
                }
                black_box((parsed_count, diagnostic_count, json_len));
                allocator.reset();
            });
        });
    }

    group.finish();
}

/// Isolated `serialize_comment_json_with_options` micro-bench — compares
/// `compat_mode = false` (default basic mode) vs `compat_mode = true`
/// (jsdoccomment-shape with `descriptionRaw` etc.). Comments are pre-parsed
/// into the arena once outside the timed loop, so the bench measures only
/// the serializer's per-mode cost.
///
/// See `design/008-oxlint-oxfmt-support/README.md` §7.4 (validation matrix).
fn bench_serializer_compat_modes(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("serializer_compat_modes");
    let fixtures = ox_jsdoc_benchmark::load_perf_fixtures();

    // Pre-build SerializeOptions instances once — we want to keep the
    // construction cost out of the timed region.
    let basic_opts = SerializeOptions::default();
    let compat_opts = SerializeOptions {
        compat_mode: true,
        ..SerializeOptions::default()
    };

    for fixture in fixtures {
        let comment_texts = &fixture.comment_texts;

        // Two passes per fixture: basic and compat. Each pre-parses into
        // its own arena so the timed loop reuses the same parsed AST set.
        for (mode_label, opts) in [("basic", &basic_opts), ("compat", &compat_opts)] {
            let id = BenchmarkId::new(format!("{}/{}", fixture.bucket, mode_label), &fixture.name);

            group.bench_function(id, |b| {
                let allocator = Allocator::default();
                // Pre-parse once outside the timed loop. Borrow lifetimes
                // tie the parsed comments to the arena, so the closure can
                // call serialize repeatedly without re-parsing.
                let parsed: Vec<_> = comment_texts
                    .iter()
                    .filter_map(|src| {
                        parse_comment(&allocator, src, 0, ParseOptions::default()).comment
                    })
                    .collect();

                b.iter(|| {
                    let mut json_len = 0usize;
                    for comment in &parsed {
                        let json = serialize_comment_json_with_options(
                            black_box(comment),
                            None,
                            None,
                            opts,
                        );
                        json_len += json.len();
                    }
                    black_box(json_len);
                });
            });
        }
    }

    group.finish();
}

criterion_group!(serializer, bench_serializer, bench_serializer_compat_modes);
criterion_main!(serializer);
