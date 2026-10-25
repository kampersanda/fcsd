#!/usr/bin/env python3
"""Combine memory CSV and Criterion estimates into a Markdown comparison."""
import argparse
import csv
import json
from pathlib import Path
import platform
import subprocess

DATASETS = ("words-10000", "words-100000", "wiki-urls-10000", "wiki-urls-100000")
IMPLEMENTATIONS = ("fcsd-4", "fcsd-8", "fcsd-16", "fcsd-32", "fst", "yada")
OPERATIONS = ("build", "locate-hit", "locate-miss", "decode", "iterate")


def supported(implementation, operation):
    if operation == "decode":
        return implementation.startswith("fcsd-")
    return operation != "iterate" or implementation != "yada"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--criterion", type=Path, default=Path("target/criterion"))
    parser.add_argument("--memory", type=Path, default=Path("bench/results/memory.csv"))
    parser.add_argument("--timings", type=Path, default=Path("bench/results/timing.csv"),
                        help="Output CSV with timing means and 95%% confidence intervals")
    parser.add_argument("--command", required=True, help="Exact timing command used")
    parser.add_argument("--measured-on", required=True, help="Measurement date (YYYY-MM-DD)")
    args = parser.parse_args()
    with args.memory.open() as file:
        memory = {(r["dataset"], r["implementation"]): r for r in csv.DictReader(file)}
    timings = {}
    intervals = {}
    for file in args.criterion.glob("**/new/benchmark.json"):
        benchmark = json.loads(file.read_text())
        parts = benchmark["full_id"].split("/")
        if len(parts) == 3:
            estimates = json.loads(file.with_name("estimates.json").read_text())
            timings[tuple(parts)] = estimates["mean"]["point_estimate"]
            intervals[tuple(parts)] = estimates["mean"]["confidence_interval"]

    # Fail instead of silently presenting an incomplete run as a comparison.
    for dataset in DATASETS:
        for implementation in IMPLEMENTATIONS:
            if (dataset, implementation) not in memory:
                parser.error(f"Missing size result: {dataset}/{implementation}")
            for operation in OPERATIONS:
                if supported(implementation, operation) and (dataset, operation, implementation) not in timings:
                    parser.error(f"Missing timing result: {dataset}/{operation}/{implementation}")

    args.timings.parent.mkdir(parents=True, exist_ok=True)
    with args.timings.open("w", newline="") as file:
        writer = csv.writer(file, lineterminator="\n")
        writer.writerow(("dataset", "operation", "implementation", "batch_mean_ns", "ci_lower_ns", "ci_upper_ns", "confidence_level"))
        for dataset in DATASETS:
            for operation in OPERATIONS:
                for implementation in IMPLEMENTATIONS:
                    if supported(implementation, operation):
                        key = (dataset, operation, implementation)
                        ci = intervals[key]
                        writer.writerow((*key, timings[key], ci["lower_bound"], ci["upper_bound"], ci["confidence_level"]))

    def command(*args):
        return subprocess.check_output(args, text=True).strip()

    cpu = platform.processor() or platform.machine()
    if platform.system() == "Darwin":
        cpu = command("sysctl", "-n", "machdep.cpu.brand_string")
    print("# Benchmark results\n")
    print(f"Measured on {args.measured_on}. CPU: {cpu}; OS: {platform.platform()}.")
    print(f"Compiler: `{command('rustc', '-V')}`. Release profile, default features.")
    print(f"Base revision: `{command('git', 'rev-parse', 'HEAD')}` (with the benchmark changes in this branch).\n")
    print("Timing command:\n\n```sh\n" + args.command + "\n```\n")
    print("Resolved direct dependencies:\n\n```text")
    print(command("cargo", "tree", "-p", "fcsd-bench", "--depth", "1", "--offline").replace(str(Path.cwd()), "."))
    print("```\n")
    print("Times are Criterion means. Build is milliseconds per dictionary; other times are nanoseconds per key/query, measured in full-dataset batches. Lower is better for every column. N/A means the library has no corresponding public operation.\n")
    print("Batch means and 95% confidence intervals are saved in [timing.csv](results/timing.csv); encoded sizes are in [memory.csv](results/memory.csv).\n")
    print("This local run is a reference, not a cross-machine guarantee. See [methodology](README.md) for workload and allocation differences.\n")
    for dataset in DATASETS:
        first = memory[dataset, IMPLEMENTATIONS[0]]
        print(f"## {dataset}\n")
        print(f"{int(first['keys']):,} unique keys; {int(first['raw_bytes']):,} raw bytes (including one terminator per key).\n")
        print("| Implementation | Bytes | Bytes/key | Size/raw | Build (ms) | Hit (ns) | Miss (ns) | Decode (ns) | Iterate (ns) |")
        print("|---|---:|---:|---:|---:|---:|---:|---:|---:|")
        for implementation in IMPLEMENTATIONS:
            row = memory[dataset, implementation]
            cells = [implementation, f"{int(row['dictionary_bytes']):,}", row['bytes_per_key'], f"{float(row['compression_ratio']):.3f}"]
            for operation in OPERATIONS:
                value = timings.get((dataset, operation, implementation))
                divisor = 1e6 if operation == "build" else int(row['keys'])
                cells.append(f"{value / divisor:,.2f}" if supported(implementation, operation) else "N/A")
            print("| " + " | ".join(cells) + " |")
        print()


if __name__ == "__main__":
    main()
