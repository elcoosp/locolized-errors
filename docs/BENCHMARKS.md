# Benchmarks — 0.3.0 vs 0.2.0

Measured on the development machine for this iteration (Linux x86_64, rustc
1.99.1, debug-profile comparison at `-O0` semantics identical for both
versions), with `criterion` 0.8. The **legacy** variants are the 0.2.0
implementations copied verbatim into
[`benches/perf.rs`](../locolized-errors/benches/perf.rs) and run against the
same enums and locale data, so the delta isolates the algorithmic changes.
Every number is the median of 100 samples after a 3 s warm-up.

## Runtime hot paths

| Benchmark | 0.2.0 (legacy) | 0.3.0 (current) | Δ | Why |
| :--- | ---: | ---: | ---: | :--- |
| `display/unit` (`to_string` on a unit variant) | 44.7 ns | 35.8 ns | **−20 %** | `Formatter::write_str` instead of `write!` formatting machinery; no `String` forced. |
| `display/interpolated` (1 named field through `t!`) | 107.9 ns | 99.3 ns | **−8 %** | Same `t!` fast path, minus the `format!` wrapper. |
| `error_kind/custom` (custom-status variant) | 11.5 ns | 0.81 ns | **14× faster** | `Cow::Borrowed("desc")` instead of `String::from(desc)` — the per-call heap allocation is gone. |
| `payload/to_json` (1 named field) | 415.9 ns | 383.1 ns | **−8 %** | Single-pass `Serialize` struct instead of `Map` assembly. |
| `payload/to_json_string` | 962.1 ns | 301.3 ns | **3.2× faster (−69 %)** | The `Value` tree + `Value::to_string` re-walk is replaced by one streaming pass. |
| `payload/to_json_bytes` | — (n/a) | 323.0 ns | new | Bytes are now the canonical payload path; same single pass, no intermediate `String`. |
| `loco_error/bad_request` (envelope) | 326.1 ns | 278.6 ns | **−15 %** | Eager `unwrap_or` fallback allocations removed; message comes from `error_message()` (`Cow`) instead of `to_string()`. |
| `into_response` (status + body collected) | 2 223 ns | 1 478 ns | **−34 %** | One pass to bytes, no `Value` tree, no `String`, no body re-copy; also fixes the `content-type` (was `text/plain`, now `application/json`). |
| `payload/error_params` (introspection) | — | 12.6 ns | — | Kept as the `Vec`-based introspection API; the payload hot paths no longer call it. |

Reading notes:

- `display` numbers include the `rust_i18n` backend lookup (`t!`), which is
  shared by both versions — the deltas above are pure library-level wins.
- `into_response` includes collecting the full HTTP body, so the −34 % covers
  the entire response-construction path a Loco/axum handler pays per error.
- The 0.2.0 `into_response` also served the JSON payload with
  `content-type: text/plain; charset=utf-8`; 0.3.0 emits
  `application/json` (correctness fix, see `docs/PERFORMANCE.md`).

## Compile time & dependency graph

Clean `cargo build --workspace` (debug, cold cache, same machine):

| Metric | 0.2.0 | 0.3.0 | Δ |
| :--- | ---: | ---: | ---: |
| Unique packages (`cargo tree -e normal --workspace`) | 402 | 344 | **−58 crates** |
| Clean build wall time | 137 s | 110 s | **−20 %** |

The package drop is the `loco-rs` `with-db` feature removal: this library only
touches `loco_rs::{errors, controller::ErrorDetail, prelude::IntoResponse}`,
none of which are feature-gated, so sea-orm, sea-orm-migration and sqlx (and
their transitive trees) no longer compile for consumers that do not enable
them themselves. Real Loco apps enable `with-db` in their own dependency, so
feature unification keeps their builds unchanged; standalone and non-Loco
consumers skip the entire database stack. Additionally, `syn` no longer pulls
`full`/`extra-traits` and `serde` no longer needs `derive` in the runtime
crate (smaller macro-crate compiles for every downstream build).

## Reproducing

```bash
cargo bench --bench perf            # criterion, legacy_* vs current API
cargo llvm-cov --workspace --all-targets --summary-only
```
