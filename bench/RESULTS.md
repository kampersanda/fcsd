# Benchmark results

Measured on 2026-10-25. CPU: Apple M5; OS: macOS-27.0.1-arm64-arm-64bit-Mach-O.
Compiler: `rustc 1.99.0 (b940084d7 2026-09-28)`. Release profile, default features.
Base revision: `b13ca5881bcb30d24c9ae0607546050008ea46fa` (with the benchmark changes in this branch).

Timing command:

```sh
cargo bench -p fcsd-bench --bench timing -- --sample-size 10 --warm-up-time 0.2 --measurement-time 1
```

Resolved direct dependencies:

```text
fcsd-bench v0.1.0 (./bench)
├── fcsd v0.3.0 (.)
│   [dev-dependencies]
├── fst v0.4.7
└── yada v0.7.0
[dev-dependencies]
└── criterion v0.8.2
```

Times are Criterion means. Build is milliseconds per dictionary; other times are nanoseconds per key/query, measured in full-dataset batches. Lower is better for every column. N/A means the library has no corresponding public operation.

Batch means and 95% confidence intervals are saved in [timing.csv](results/timing.csv); encoded sizes are in [memory.csv](results/memory.csv).

This local run is a reference, not a cross-machine guarantee. See [methodology](README.md) for workload and allocation differences.

## words-10000

10,000 unique keys; 121,509 raw bytes (including one terminator per key).

| Implementation | Bytes | Bytes/key | Size/raw | Build (ms) | Hit (ns) | Miss (ns) | Decode (ns) | Iterate (ns) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fcsd-4 | 109,882 | 10.988 | 0.904 | 0.14 | 122.56 | 141.47 | 46.59 | 28.49 |
| fcsd-8 | 104,370 | 10.437 | 0.859 | 0.15 | 129.21 | 132.94 | 61.11 | 28.85 |
| fcsd-16 | 101,645 | 10.165 | 0.837 | 0.16 | 145.22 | 149.97 | 86.53 | 28.93 |
| fcsd-32 | 100,228 | 10.023 | 0.825 | 0.15 | 179.13 | 183.21 | 127.33 | 29.20 |
| fst | 120,124 | 12.012 | 0.989 | 3.49 | 101.20 | 103.82 | N/A | 91.22 |
| yada | 357,376 | 35.738 | 2.941 | 82.43 | 14.90 | 32.26 | N/A | N/A |

## words-100000

100,001 unique keys; 1,205,401 raw bytes (including one terminator per key).

| Implementation | Bytes | Bytes/key | Size/raw | Build (ms) | Hit (ns) | Miss (ns) | Decode (ns) | Iterate (ns) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fcsd-4 | 999,651 | 9.996 | 0.829 | 2.46 | 204.84 | 223.51 | 54.24 | 30.85 |
| fcsd-8 | 923,720 | 9.237 | 0.766 | 2.37 | 221.84 | 227.03 | 76.20 | 31.15 |
| fcsd-16 | 885,797 | 8.858 | 0.735 | 2.32 | 248.29 | 248.60 | 114.96 | 30.83 |
| fcsd-32 | 866,735 | 8.667 | 0.719 | 2.92 | 309.67 | 314.67 | 195.58 | 30.75 |
| fst | 1,073,892 | 10.739 | 0.891 | 25.12 | 145.46 | 152.45 | N/A | 85.65 |
| yada | 3,004,416 | 30.044 | 2.492 | 618.28 | 21.07 | 47.57 | N/A | N/A |

## wiki-urls-10000

10,000 unique keys; 487,757 raw bytes (including one terminator per key).

| Implementation | Bytes | Bytes/key | Size/raw | Build (ms) | Hit (ns) | Miss (ns) | Decode (ns) | Iterate (ns) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fcsd-4 | 244,701 | 24.470 | 0.502 | 0.38 | 426.09 | 455.82 | 67.77 | 37.48 |
| fcsd-8 | 200,440 | 20.044 | 0.411 | 0.37 | 442.89 | 459.28 | 93.32 | 34.49 |
| fcsd-16 | 178,410 | 17.841 | 0.366 | 0.37 | 472.45 | 482.64 | 122.09 | 34.43 |
| fcsd-32 | 167,255 | 16.726 | 0.343 | 0.37 | 533.64 | 516.02 | 189.59 | 34.46 |
| fst | 181,236 | 18.124 | 0.372 | 6.12 | 204.34 | 205.47 | N/A | 148.30 |
| yada | 585,728 | 58.573 | 1.201 | 122.36 | 94.41 | 128.43 | N/A | N/A |

## wiki-urls-100000

97,054 unique keys; 4,740,998 raw bytes (including one terminator per key).

| Implementation | Bytes | Bytes/key | Size/raw | Build (ms) | Hit (ns) | Miss (ns) | Decode (ns) | Iterate (ns) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fcsd-4 | 2,247,495 | 23.157 | 0.474 | 4.25 | 528.09 | 556.02 | 71.50 | 38.40 |
| fcsd-8 | 1,785,613 | 18.398 | 0.377 | 4.17 | 544.44 | 551.42 | 95.25 | 37.53 |
| fcsd-16 | 1,556,573 | 16.038 | 0.328 | 4.13 | 584.85 | 577.16 | 144.15 | 35.99 |
| fcsd-32 | 1,442,134 | 14.859 | 0.304 | 4.09 | 659.40 | 663.17 | 235.74 | 35.84 |
| fst | 1,534,172 | 15.807 | 0.324 | 47.95 | 256.31 | 253.76 | N/A | 130.98 |
| yada | 4,931,584 | 50.813 | 1.040 | 958.58 | 103.28 | 149.37 | N/A | N/A |

