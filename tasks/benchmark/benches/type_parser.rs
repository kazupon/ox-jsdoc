// @author kazuya kawaguchi (a.k.a. kazupon)
// @license MIT
//

use ox_jsdoc::{ParseMode, ParseOptions, parse_comment, parse_type};
use ox_jsdoc_benchmark::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use oxc_allocator::Allocator;

const TYPE_EXPRESSIONS: &[(&str, &str)] = &[
    ("basic/name", "string"),
    ("basic/null", "null"),
    ("basic/any", "*"),
    ("basic/unknown", "?"),
    ("union/two", "string | number"),
    ("union/three", "string | number | boolean"),
    ("union/complex", "string | number | null | undefined | boolean"),
    ("intersection/two", "A & B"),
    ("intersection/three", "A & B & C"),
    ("generic/simple", "Array<string>"),
    ("generic/nested", "Map<string, Array<number>>"),
    ("generic/dot", "Object.<string, number>"),
    ("array/bracket", "string[]"),
    ("array/nested", "number[][]"),
    ("modifier/nullable-pre", "?string"),
    ("modifier/nullable-post", "string?"),
    ("modifier/not-nullable", "!Object"),
    ("modifier/optional", "string="),
    ("modifier/variadic", "...string"),
    ("function/no-params", "function()"),
    ("function/params-return", "function(string, number): boolean"),
    ("function/arrow", "(x: number) => string"),
    ("function/arrow-multi", "(a: string, b: number, c: boolean) => void"),
    ("object/empty", "{}"),
    ("object/simple", "{key: string}"),
    ("object/multi", "{a: string, b: number, c: boolean}"),
    ("object/optional", "{a?: string, b?: number}"),
    ("ts/conditional", "T extends U ? X : Y"),
    ("ts/conditional-complex", "T extends Array<infer U> ? U : never"),
    ("ts/keyof", "keyof MyType"),
    ("ts/typeof", "typeof myVar"),
    ("ts/import", "import('./module')"),
    ("ts/predicate", "x is string"),
    ("ts/asserts", "asserts x is T"),
    ("ts/readonly-array", "readonly string[]"),
    ("ts/unique-symbol", "unique symbol"),
    ("tuple/two", "[string, number]"),
    ("tuple/labeled", "[a: string, b: number]"),
    ("complex/union-generic", "Array<string> | Map<string, number> | null"),
    ("complex/nested-generic", "Promise<Map<string, Array<number>>>"),
    ("complex/conditional-keyof", "K extends keyof T ? T[K] : never"),
    ("realworld/react-props", "{children: React.ReactNode, className?: string}"),
    ("realworld/union-literals", "\"success\" | \"error\" | \"pending\""),
];

/// Benchmark parse_type (type parser only, no comment parser overhead).
fn bench_parse_type(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parse_type");

    for &(name, type_expr) in TYPE_EXPRESSIONS {
        let id = BenchmarkId::new("type_only", name);
        group.bench_function(id, |b| {
            let mut allocator = Allocator::default();
            b.iter(|| {
                let _ = parse_type(
                    &allocator,
                    black_box(type_expr),
                    0,
                    ParseMode::Typescript,
                );
                allocator.reset();
            });
        });
    }

    group.finish();
}

/// Benchmark parse_comment with parse_types enabled (full pipeline).
fn bench_parse_comment_with_types(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("parse_comment_with_types");

    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    };

    for &(name, type_expr) in TYPE_EXPRESSIONS {
        let comment = format!("/** @param {{{type_expr}}} x */");
        let id = BenchmarkId::new("full_pipeline", name);
        group.bench_function(id, |b| {
            let mut allocator = Allocator::default();
            b.iter(|| {
                let _ = parse_comment(
                    &allocator,
                    black_box(&comment),
                    0,
                    options,
                );
                allocator.reset();
            });
        });
    }

    group.finish();
}

/// Batch: all types at once.
fn bench_batch(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("type_parser_batch");

    // Type-only batch
    let id = BenchmarkId::new("type_only", format!("{}_types", TYPE_EXPRESSIONS.len()));
    group.bench_function(id, |b| {
        let mut allocator = Allocator::default();
        b.iter(|| {
            for &(_, type_expr) in TYPE_EXPRESSIONS {
                let _ = parse_type(
                    &allocator,
                    black_box(type_expr),
                    0,
                    ParseMode::Typescript,
                );
            }
            allocator.reset();
        });
    });

    // Full pipeline batch
    let comments: Vec<String> = TYPE_EXPRESSIONS.iter()
        .map(|&(_, type_expr)| format!("/** @param {{{type_expr}}} x */"))
        .collect();
    let options = ParseOptions {
        parse_types: true,
        type_parse_mode: ParseMode::Typescript,
        ..ParseOptions::default()
    };
    let id = BenchmarkId::new("full_pipeline", format!("{}_types", TYPE_EXPRESSIONS.len()));
    group.bench_function(id, |b| {
        let mut allocator = Allocator::default();
        b.iter(|| {
            for comment in &comments {
                let _ = parse_comment(
                    &allocator,
                    black_box(comment),
                    0,
                    options,
                );
            }
            allocator.reset();
        });
    });

    // Baseline: no type parsing
    let options_no_types = ParseOptions::default();
    let id = BenchmarkId::new("no_types_baseline", format!("{}_types", TYPE_EXPRESSIONS.len()));
    group.bench_function(id, |b| {
        let mut allocator = Allocator::default();
        b.iter(|| {
            for comment in &comments {
                let _ = parse_comment(
                    &allocator,
                    black_box(comment),
                    0,
                    options_no_types,
                );
            }
            allocator.reset();
        });
    });

    group.finish();
}

/// Mode comparison for type-only parsing.
fn bench_modes(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("type_parser_modes");

    let type_expr = "Array.<string> | Map.<string, number>";
    for (mode_name, mode) in [
        ("jsdoc", ParseMode::Jsdoc),
        ("closure", ParseMode::Closure),
        ("typescript", ParseMode::Typescript),
    ] {
        let id = BenchmarkId::new("type_only", mode_name);
        group.bench_function(id, |b| {
            let mut allocator = Allocator::default();
            b.iter(|| {
                let _ = parse_type(
                    &allocator,
                    black_box(type_expr),
                    0,
                    mode,
                );
                allocator.reset();
            });
        });
    }

    group.finish();
}

criterion_group!(type_parser, bench_parse_type, bench_parse_comment_with_types, bench_batch, bench_modes);
criterion_main!(type_parser);
