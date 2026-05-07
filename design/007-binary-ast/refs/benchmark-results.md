# Benchmark Results (2026-04-10)

Environment: Apple M1 Max, Node.js 24.14.1, Rust 1.92.0

## Rust Parser (ox-jsdoc core, criterion)

| Fixture                                  |     Avg |
| ---------------------------------------- | ------: |
| `common/basic-param`                     |  422 ns |
| `description-heavy/linkcode-description` |  921 ns |
| `malformed/unclosed-inline-tag`          |  400 ns |
| `source/typed-api-client` (3 blocks)     | 1.74 µs |
| `source/vue-i18n-composer` (5 blocks)    | 2.74 µs |
| `special-tag/memberof-borrows`           |  469 ns |
| `toolchain/vue-i18n-custom-tags`         |  928 ns |
| `type-heavy/ts-import-record-type`       |  499 ns |

## Rust Parser vs comment-parser

| Fixture                                  | ox-jsdoc Rust | comment-parser JS | Speedup |
| ---------------------------------------- | ------------: | ----------------: | ------: |
| `common/basic-param`                     |        422 ns |          3,626 ns |    8.6x |
| `description-heavy/linkcode-description` |        921 ns |          3,578 ns |    3.9x |
| `malformed/unclosed-inline-tag`          |        400 ns |          1,989 ns |    5.0x |
| `source/typed-api-client` (3 blocks)     |      1,740 ns |         15,400 ns |    8.9x |
| `source/vue-i18n-composer` (5 blocks)    |      2,740 ns |         19,125 ns |    7.0x |
| `special-tag/memberof-borrows`           |        469 ns |          4,285 ns |    9.1x |
| `toolchain/vue-i18n-custom-tags`         |        928 ns |          6,927 ns |    7.5x |
| `type-heavy/ts-import-record-type`       |        499 ns |          4,529 ns |    9.1x |

## JS Binding: Transfer Method Comparison (mitata)

| Fixture                          | JSON string | JSON buffer | Direct object | comment-parser |
| -------------------------------- | ----------: | ----------: | ------------: | -------------: |
| `common/basic-param`             |     81.4 µs |     84.8 µs |       30.0 µs |         3.7 µs |
| `description-heavy/linkcode`     |    101.4 µs |    107.5 µs |       30.9 µs |         3.6 µs |
| `type-heavy/ts-import-record`    |     48.7 µs |     50.2 µs |       16.3 µs |         4.6 µs |
| `special-tag/memberof-borrows`   |     74.5 µs |     75.4 µs |       28.1 µs |         4.2 µs |
| `malformed/unclosed-inline-tag`  |     47.1 µs |     49.3 µs |       19.1 µs |         2.0 µs |
| `source/typed-api-client`        |    329.8 µs |    337.7 µs |      118.2 µs |        15.2 µs |
| `source/vue-i18n-composer`       |    415.1 µs |    420.0 µs |      143.3 µs |        19.2 µs |
| `toolchain/vue-i18n-custom-tags` |    158.1 µs |    159.1 µs |       54.2 µs |         7.0 µs |

Findings:

- JSON buffer (serde_json::to_writer) shows no improvement over JSON string — bottleneck is JSON.parse on JS side
- Direct object (#[napi(object)]) is 2.5-3x faster than JSON transfer
- All NAPI methods are still slower than comment-parser due to V8 object construction overhead

## JS Binding: parse vs parseMultiple (source fixtures, mitata)

| Fixture | parse (one-by-one) | parseMultiple (batch) | comment-parser | Batch improvement |
| --- | --: | --: | --: | --: |
| `source/typed-api-client` (3 blocks) | 308 µs | 119 µs | 15.9 µs | 2.6x |
| `source/vue-i18n-composer` (5 blocks) | 406 µs | 142 µs | 19.5 µs | 2.9x |

Findings:

- parseMultiple reduces NAPI crossing from N calls to 1 call
- ~2.7x improvement over one-by-one parse
- Still 7-8x slower than comment-parser

## Summary

| Layer                    | vs comment-parser | Use case                           |
| ------------------------ | ----------------- | ---------------------------------- |
| Rust parser              | **4-9x faster**   | oxlint integration, Rust toolchain |
| JS parseMultiple (batch) | **7-8x slower**   | Batch processing, ESLint plugin    |
| JS parse (one-by-one)    | **16-21x slower** | Individual comment parsing         |

The Rust parser is very fast. The JS binding bottleneck is NAPI object construction overhead, not parsing. The value of ox-jsdoc's JS binding is integration with the Rust ecosystem, not raw JS-side speed.
