// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use ox_jsdoc::{
    ParseOptions, ValidationOptions, analyze_comment, parse_comment, serialize_comment_json,
    validate_comment,
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

criterion_group!(serializer, bench_serializer);
criterion_main!(serializer);
