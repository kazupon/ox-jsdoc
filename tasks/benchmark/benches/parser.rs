// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use ox_jsdoc::{ParseOptions, parse_comment};
use ox_jsdoc_benchmark::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxc_allocator::Allocator;

fn bench_parser(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parser");
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
                black_box(&parsed);
                allocator.reset();
            });
        });
    }

    group.finish();
}

criterion_group!(parser, bench_parser);
criterion_main!(parser);
