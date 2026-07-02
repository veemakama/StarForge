# TODO — #314 D-17 Add Contract Performance Profiling

- [ ] Implement deterministic memory tracking in `perf record`:
  - Add CLI flag `--memory-bytes <u64>`.
  - Store into `GasUsageRecord`/snapshots.
- [ ] Update performance logic in `src/utils/performance.rs`:
  - Add memory handling to bottleneck identification.
  - Add memory regression detection.
  - Add memory comparison metrics.
  - Extend dashboard output/structures with memory summary.
- [ ] Improve execution-time regression detection (time-only regression metrics + points).
- [ ] Update unit tests in `src/utils/performance.rs` to cover:
  - memory recording + retrieval
  - bottleneck analysis with memory
  - regression detection using memory/time
  - comparison snapshots include memory
- [ ] Run `cargo test` and fix any compile/test failures.

