# Dictionary benchmarks

Compare fcsd (bucket sizes 4, 8, 16, 32), [fst](https://github.com/BurntSushi/fst),
and [yada](https://github.com/takuyaa/yada) on the four datasets already in `data/`.
See [measured results](RESULTS.md) for a complete comparison, including the machine,
compiler, dependency versions, and timing command.

## Run

Run these commands from the repository root. Python 3 is needed only for the report.
Dataset paths are resolved relative to the benchmark crate, independently of the
working directory. The workspace requires Rust 1.86 or newer for Criterion.

```sh
cargo test -p fcsd-bench --release
mkdir -p bench/results
cargo run --release -p fcsd-bench --bin memory > bench/results/memory.csv
cargo bench -p fcsd-bench --bench timing
python3 bench/report.py --measured-on YYYY-MM-DD \
  --command 'cargo bench -p fcsd-bench --bench timing' > bench/RESULTS.md
```

Criterion writes estimates, confidence intervals, and HTML reports under
`target/criterion/` (open `target/criterion/report/index.html`). Its normal defaults
are 100 samples, 3 seconds of warmup and 5 seconds of measurement per case.
For a shorter run, pass `-- --sample-size 10 --warm-up-time 0.2 --measurement-time 1`.
Record those arguments in `--command` when generating the report.
Use a fresh `CARGO_TARGET_DIR` for a clean measurement run and pass its
`criterion/` directory to `report.py --criterion PATH`; this avoids mixing old and
new estimates. A filtered Criterion run cannot produce a complete comparison by
itself. The report rejects missing cases, but cannot detect mixed runs.

`memory` emits CSV, and `report.py` combines it with Criterion's mean estimates.
The report also exports batch means and 95% confidence intervals to
`bench/results/timing.csv` (override with `--timings PATH`).
The timing suite has 108 cases across four datasets. Keep `Cargo.lock` alongside
local results to reproduce dependency resolution (it is ignored by this library
repository). The recorded direct versions appear in the generated report.

## Methodology

Inputs are UTF-8 lines, sorted and deduplicated before measurement. Actual unique
counts are reported: `words-100000` has 100,001 keys and `wiki-urls-100000` has
97,054, despite their filenames. All structures
map each key to its lexicographic ID: fcsd stores that rank implicitly, fst uses a
`Map` with `u64` values, and yada stores `u32` values. These datasets satisfy yada's
non-NUL key and 31-bit value restrictions. Dataset loading, sorting, query
preparation, and lookup dictionary construction are outside timed regions.

| Metric | Workload |
|---|---|
| Build | Construct and drop a complete owned dictionary from sorted keys. The borrowed key/value list for yada is prepared beforehand, like the pre-existing key vector used by fcsd/fst; fst creates its iterator inside the timed region. Includes yada's checked constructor. |
| Locate hit | Search every stored key once, in a fixed-seed shuffled order. |
| Locate miss | Search one absent query per key, in the same shuffled order. Append `~` to the key, repeating it if necessary to guarantee absence. This measures misses sharing a full stored-key prefix, not arbitrary random strings. |
| Decode | Decode every lexicographic ID once in shuffled order. fcsd only; fst and yada expose no direct ID-to-key operation. |
| Iterate | Enumerate all key/ID pairs in lexicographic order. fcsd and fst only; yada exposes no full-key iterator. fcsd returns owned key buffers while fst lends keys from its stream, so allocation costs differ. |
| Dictionary bytes | Encoded dictionary length, including format metadata. Excludes source keys, query buffers, wrapper structs, spare allocation capacity, and temporary construction memory; this is neither peak memory nor process RSS. |
| Bytes/key | Dictionary bytes divided by the number of unique keys. |
| Size/raw | Dictionary bytes divided by the sum of UTF-8 key lengths plus one terminator byte per key. Lower is better; values above 1 mean expansion. |

Each timed query/decode/iteration batch visits all keys; the Markdown report divides
batch time by key count. Criterion throughput uses the same count. Each result is
passed through `std::hint::black_box`, including misses, to retain the measured
work. Locator/decoder scratch buffers are reused across batches. Decode and fcsd
iteration include allocation and destruction of the returned key buffers. Queries
are repeatable and warm-cache; these measurements do not model cold disk access,
concurrent requests, or tail latency. Run on an idle machine for useful comparisons.

The benchmark tests verify that every dictionary returns the same IDs, all miss
queries are absent, and fcsd decodes the original keys for every bucket size on all
four datasets. Unsupported operations are N/A, without auxiliary reverse indexes.
Prefix search is excluded because the public operations differ: fcsd/fst support
predictive enumeration, whereas yada provides common-prefix search.

## Credit

The datasets under `data` are copied from the [fst crate](https://github.com/BurntSushi/fst).
