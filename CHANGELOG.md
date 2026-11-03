## [0.3.0] - 2026-11-03

### Features

- Add `error_message()` to the trait: the localized message as `Cow<'static, str>`, borrowed when no interpolation is needed
- Add `to_json_bytes()` for a zero-copy route from payload to HTTP body
- [**breaking**] Extend the named kinds with `forbidden`, `conflict`, `unprocessable-entity`, `too-many-requests` and `service-unavailable`
- Stream every JSON payload in a single pass from the variant fields (no intermediate `serde_json::Value` tree, no per-field materialization)
- Re-export `axum` for downstream use of the generated response path

### Bug Fixes

- Generated `IntoResponse` now serves the payload with `content-type: application/json` (0.2.0 unintentionally served it as `text/plain`)

### Performance

- [**breaking**] `ErrorKind::Custom(StatusCode, Cow<'static, str>)`: macro-generated kinds hold borrowed descriptions — no per-call allocation (14× faster `error_kind()`)
- `to_json_string()` 3.2× faster, `into_response()` 34 % faster, `to_loco_error()` 15 % faster, `Display` 20 % faster (see docs/BENCHMARKS.md)
- [**breaking**] Drop the `loco-rs` `with-db` feature and trim `syn`/`serde` features: 58 fewer crates, ~20 % faster clean builds for non-Loco consumers
- Eager fallback allocations removed from the BadRequest envelope

### Documentation

- Add docs/PERFORMANCE.md (findings and design) and docs/BENCHMARKS.md (methodology and measured results)
- README: nine-kind reference, trait surface, performance section, 0.3.0 breaking changes

### Tests

- 24 new tests: streaming payloads, `application/json` contract, `error_message()` semantics, extended kinds, fallback degradation, hand-written implementors
- Keep 100 % line coverage of the workspace (cargo-llvm-cov, 0 uncovered rows)
- Add criterion benchmark suite comparing 0.2.0 implementations verbatim against the 0.3.0 API

## [0.2.0] - 2026-11-01

### Features

- Init
- If bad request send self as code & message
- [**breaking**] Redesign derive with helper attributes and panic-free diagnostics
- Add structured payloads, IntoResponse and trait introspection

### Bug Fixes

- Error kind

### Documentation

- Add readme
- Rewrite README with verified examples and record the improvement brainstorm

### Tests

- Cover every macro path and runtime behavior

### Miscellaneous Tasks

- Add desc
- Readme
- [**breaking**] Update all deps to latest majors and centralize workspace metadata
- Add pedantic CI, llvm-cov coverage and git-cliff release automation
