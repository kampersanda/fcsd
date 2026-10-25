# Dictionary benchmarks

Compare fcsd (bucket sizes 4, 8, 16, 32), [fst](https://github.com/BurntSushi/fst),
and [yada](https://github.com/takuyaa/yada) on the four word and Wikipedia URL datasets in `data/`.

**[View results](RESULTS.md)** for the comparison tables, measurement environment,
and CSV files with 95% confidence intervals.

## Reading the results

All implementations map keys to lexicographic IDs. fst uses `Map` with `u64`
values, rather than `Set`; yada uses `u32` values.

| Column | Meaning |
|---|---|
| Bytes | Encoded dictionary size, including metadata; excludes construction memory and input buffers. |
| Bytes/key | Dictionary size divided by the number of unique keys. |
| Size/raw | Dictionary size divided by UTF-8 input size, counting one terminator byte per key. |
| Build (ms) | Time to construct and drop one dictionary. |
| Hit / Miss (ns) | Mean lookup time per present / absent query. |
| Decode (ns) | Mean time to recover a key from its ID. |
| Iterate (ns) | Mean time per key when enumerating the whole dictionary. |

Lower is better in every column. `N/A` means the operation is unsupported.

Measurement notes:

- Keys are sorted and deduplicated. Input/query preparation is outside timing;
  yada's key/value list is prepared beforehand, but its checked constructor is timed.
- Lookups and decoding use a fixed shuffled order. Misses append `~` to stored
  keys until absent. Iteration visits all keys in order.
- fcsd decoding and iteration allocate returned keys; fst iteration borrows keys.
  All measurements use repeated in-memory access.

## Run

Requires Rust 1.86+ and Python 3. Run from the repository root:

```sh
cargo test -p fcsd-bench --release
mkdir -p bench/results
cargo run --release -p fcsd-bench --bin memory > bench/results/memory.csv
cargo bench -p fcsd-bench --bench timing
python3 bench/report.py --measured-on YYYY-MM-DD \
  --command 'cargo bench -p fcsd-bench --bench timing' > bench/RESULTS.md
```

Replace `YYYY-MM-DD` with the measurement date. The script generates the Markdown
tables and `bench/results/timing.csv` from Criterion's JSON; no manual transcription
is needed. Criterion's HTML report is at `target/criterion/report/index.html`.

For the shorter run used in the published results, append
`-- --sample-size 10 --warm-up-time 0.2 --measurement-time 1` to the timing command
and include those arguments in `--command`.

Run all 108 cases together. To avoid mixing runs, use a fresh `CARGO_TARGET_DIR`
and pass its `criterion/` directory to `report.py --criterion PATH`.
Keep `Cargo.lock` to reproduce dependency versions.

## Credit

The datasets are copied from the [fst crate](https://github.com/BurntSushi/fst).
