# Routing Performance Benchmark Policy

This document defines the baseline performance thresholds and regression monitoring policies for the StellarRoute routing engine.

## Regression Threshold Policy

We monitor the **p50** (median) and **p95** latencies for critical pathfinding routes (e.g., standard swaps, 4-hop routes, and cross-venue arbitrage).

### Thresholds
- **Warning Threshold**: A PR introduces a latency increase of `> 5%` in either the p50 or p95 metric compared to the `baseline_report.json`.
- **Failing Threshold (Regression)**: A PR introduces a latency increase of `> 10%` for p50, or `> 15%` for p95 metrics. Any PR exceeding this threshold MUST be optimized or explicitly approved as a known, justified degradation (e.g., a major feature addition).

### Benchmark Environment
- Benchmarks are run on the CI environment using standard graph fixtures located under `crates/routing/fixtures/`.
- CI runs `cargo bench -p stellarroute-routing` using Criterion to capture robust measurement distributions.

### Exporting and Updating Baselines
- The p50 and p95 metrics are captured by a CI step that parses Criterion's JSON output (or via the custom `bench_custom_metrics` export feature).
- To update the baseline, run the benchmark suite and commit the new generated `baseline_report.json` to the `<root>/docs` folder.

All future routing changes must adhere to the bounds established above.
