# Empirical Performance Benchmarks (`hypit-rs`)

See [BENCHMARKS.md](../BENCHMARKS.md) for full methodology and `.svml` DAG compiler metrics.

| Subsystem / Operation | Node.js / TS Baseline | `hypit-rs` (Rust) | Speedup |
| :--- | ---: | ---: | ---: |
| `.svml` AST & Speaker Turn Parsing (500 docs) | 412.60 ms | **6.40 ms** | **64.5x** |
| Phonetic `<orig\|spoken>` & `@cue` Extraction | 184.20 ms | **3.15 ms** | **58.5x** |
| DAG Compilation & Critical-Path DP Analysis | 295.80 ms | **4.80 ms** | **61.6x** |
| SHA-256 Content-Addressable Task Graph Hashing | 218.40 ms | **3.75 ms** | **58.2x** |
| **End-to-End `.svml` Compile + Kernel Cache Verification** | **1,111.00 ms** | **18.10 ms** | **61.4x** |
