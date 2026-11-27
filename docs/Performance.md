Lint avoids repeated Markdown parsing, source declaration parsing, and glob compilation within one command; a reproducible benchmark measures end-to-end behavior on a wiki with many references to the same source file.

## Benchmark

Build an optimized executable, then run the fixture benchmark:

```sh
cargo build --locked --release
python3 scripts/benchmark-lint.py target/release/awiki
```

The [benchmark script](../scripts/benchmark-lint.py) creates 100 pages, 1,000
code references, one TypeScript source with 200 methods, and 10 path overrides.
It warms up once and reports the median of three new CLI processes. Use
`--rounds <n>` to collect more samples.

On an Apple M2 with Rust 1.100.0, the same fixture measured:

| Implementation | Median wall time |
| --- | --- |
| Before shared parsing and per-run reuse | 1.414 s |
| After shared parsing and per-run reuse | 0.033 s |

This repeated-reference workload improved by about 43 times. Other wiki sizes
and link distributions need their own measurements; these results are not a
latency guarantee.

## Lifetime and tradeoffs

[`Markdown::parse()`](../src/wiki/markdown.rs#Markdown::parse) collects source
ranges once during document loading. A command retains document text and syntax
for its lifetime so lint and rename reuse the original parse. Format reuses link
ranges, then refreshes body ranges after normalization changes offsets. Memory
use grows with loaded wiki text and link counts.

[`SourceCache::check()`](../src/wiki/source.rs#SourceCache::check) reuses parsed
declarations and errors by canonical source path within one lint run. A new
command starts fresh, and glob sets are compiled once per policy. No persistent
cache or cross-run invalidation is involved.

The pre-commit hook uses a development build; release-mode validation remains
in CI and the full local checks described in [Testing](Testing.md).
