# Performance: initial Rust port

Decision: retain virtualization and background operations. **Measured:** rendered
row count stays bounded by the viewport at 1k/10k/100k mods, and a 100k search takes
about 1 ms on the development machine. This is a Rust baseline, not evidence of an
improvement over Electron: the original was not profiled with the same corpus.

## Contract and architecture

- The primary goal is responsive search, toggles and scrolling with large libraries.
- The corpus is `Catalog::demo`: 1,000 / 10,000 / 100,000 synthetic records with mixed
  Russian/English titles, unique pack names and Workshop IDs; not real pack files.
- `uniform_list` constructs visible rows only; the test requires `0 < rendered_rows < 100`.
- Rows are 40 px high; immutable catalog/order snapshots are shared through `Arc`.
- Search uses pre-normalized text, a 75 ms debounce and generation tracking. Stale
  results cannot replace newer results; previous waiting tasks are cancelled.
- Directory traversal, pack-index reads, conflicts, imports and saves run outside
  the GUI thread. Scan/conflict jobs are cancellable; one such job runs at a time.
- Rendering does not scan, sort or clone the entire catalog. Bulk commands and preset
  application may copy state snapshots; their p95 has not been measured separately.
  Conflict-checking memory depends on the number of unique file paths.
- Switching language updates UI text without rebuilding the catalog or repeating
  background work. Row formatting allocates only the selected translation.

## Environment and reproduction

Measured on 2026-10-02: Apple M5 Pro, macOS 26.6.2, Rust 1.98.1, aarch64, system
allocator. This compiler version identifies the measurement environment; the project
tracks `stable` without a compiler-version pin.

```sh
cargo bench -p wh3-core --bench catalog --locked
cargo test -p wh3-mod-manager --test visual --locked
cargo test --release -p wh3-mod-manager --test visual --locked
```

Search: 100 repetitions, catalog construction measured separately; result allocation
is included. The query is the Cyrillic word for Kislev followed by `00`; results are
consumed through `black_box`. UI: headless Metal, 1280×820 window, warmup, then 50 forced
redraws. Timings cover the CPU draw call, not full input-to-photon latency or GPU/vsync
completion. Catalog loading and setup are excluded from frame timings. The test also
exercises actual checkbox clicks, the enabled filter and last-record search through
Ctrl/Cmd+F. It verifies English/Russian switching and captures both languages.

## Results

**Measured release search:**

| Mods | Catalog build, ms | Search p50, ms | Search p95, ms |
|---:|---:|---:|---:|
| 1,000 | 1.458 | 0.017 | 0.019 |
| 10,000 | 14.183 | 0.155 | 0.282 |
| 100,000 | 67.132 | 0.710 | 1.155 |

**Measured debug frames, after column alignment:**

| Mods | Frame p50, ms | Frame p95, ms |
|---:|---:|---:|
| 1,000 | 11.891 | 12.915 |
| 10,000 | 12.536 | 15.150 |
| 100,000 | 11.849 | 13.370 |

**Measured release GPUI/Metal frames (before localization):**

| Mods | Frame p50, ms | Frame p95, ms |
|---:|---:|---:|
| 1,000 | 0.900 | 0.977 |
| 10,000 | 0.893 | 0.934 |
| 100,000 | 0.912 | 1.053 |

Raw samples: [search](measurements/query-m5-pro.csv),
[frames](measurements/frames-m5-pro.csv). The release UI measurement ran directly
from the already-built test executable, without concurrent compilation.
`/usr/bin/time -l` measured peak process RSS of **161,234,944 bytes (153.8 MiB)** and
peak memory footprint of **233,227,008 bytes**. This covers the complete sequential
run of all three sizes with Metal and screenshots; it is not a separate measurement
of memory retained after a burst.

A follow-up release run after English/Russian localization produced:

| Mods | Frame p50, ms | Frame p95, ms | Search p50, ms | Search p95, ms |
|---:|---:|---:|---:|---:|
| 1,000 | 0.911 | 0.961 | 0.017 | 0.032 |
| 10,000 | 0.912 | 0.996 | 0.139 | 0.190 |
| 100,000 | 0.904 | 0.929 | 0.806 | 1.242 |

Raw samples: [localized frames](measurements/frames-localized-m5-pro.csv),
[repeated search](measurements/query-localized-m5-pro.csv). The frame loop uses the
English interface; language-switch tests and Russian screenshots follow it. The full
test process, now capturing both locales, peaked at **178,225,152 bytes (170.0 MiB)**
RSS and **267,092,736 bytes** memory footprint. These runs do not isolate the cost of
localization from measurement noise and the additional screenshots.

Debug values check how work scales, not the speed of the shipped executable. Shared
CI runners have no hard timing threshold: structural virtualization checks are
required and numerical samples are retained for future comparison.

## Correctness and boundaries

Tests cover PFH5 and hashed-name indexes, truncated/corrupt packs, duplicate names,
preset order/state, Unicode search, preservation of the original launch script,
v3/legacy metadata export, binary round trips, CRC/bytecheck, backups and the WHM1
fixture. Native UI measurements were made on macOS. Windows GPU/driver performance,
real WH3 launch, a large real Workshop scan, post-burst RSS and speed relative to
Electron remain **unmeasured**. No SIMD, custom allocator or application unsafe code
was introduced.

The next baseline should use real Windows libraries: cold/warm startup, scrolling,
rapid typing, preset switching and conflict scans. Measure p95/p99 UI latency, peak
memory and retained memory separately from throughput.
