// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use ox_jsdoc::{ParseOptions, ValidationOptions, parse_comment, validate_comment};
use ox_jsdoc_benchmark::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxc_allocator::Allocator;

fn bench_validator(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parser_plus_validator");
    let fixtures = ox_jsdoc_benchmark::load_perf_fixtures();

    for fixture in fixtures {
        let id = BenchmarkId::new(&fixture.bucket, &fixture.name);
        let source_text = &fixture.source_text;

        group.bench_function(id, |b| {
            let mut allocator = Allocator::default();

            b.iter(|| {
                let parsed = parse_comment(
                    &allocator,
                    black_box(source_text),
                    0,
                    ParseOptions::default(),
                );
                if let Some(comment) = parsed.comment.as_ref() {
                    let validation = validate_comment(comment, ValidationOptions::default());
                    black_box(&validation);
                }
                black_box(&parsed);
                allocator.reset();
            });
        });
    }

    group.finish();
}

criterion_group!(validator, bench_validator);
criterion_main!(validator);
