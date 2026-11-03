# Brainstorm: Improvements for `locolized-errors`

Status legend: **[x]** implemented in this iteration · **[ ]** deliberately deferred (with reason).

> **0.3.0 iteration** — the second pass over this list (runtime performance,
> streaming payloads, dependency diet) is tracked at the bottom, in
> "8. Performance iteration (0.3.0)". Findings and design notes:
> [PERFORMANCE.md](PERFORMANCE.md); measured results:
> [BENCHMARKS.md](BENCHMARKS.md).

## 1. Correctness & robustness

- **[x] Panic-free macro.** The derive used `panic!` for unknown error kinds and
  non-enum inputs, which produces a compiler backtrace instead of a friendly
  error. All macro failures now emit `syn::Error::to_compile_error()` pointing
  at the offending span, with actionable hints (e.g. "add `#[localized(kind =
  \"...\")]` or `#[localized(status = 418)]`").
- **[x] README documented `{{ placeholder }}` interpolation, but rust-i18n 4.x
  only interpolates `%{placeholder}`.** Docs, locales and tests now use the
  real syntax.
- **[x] README claimed "full support for tuple variants with automatic field
  interpolation" — false.** Tuple variants cannot use rust-i18n's named
  parameters (`name = value` requires identifiers). The limitation is now
  documented honestly; named fields remain the interpolation path.
- **[x] Missing `Serialize` in every README example** while the trait has a
  `Serialize` supertrait — every documented example failed to compile. Fixed.
- **[x] Duplicate/contradictory attribute handling** is now a compile error
  instead of silent last-wins.

## 2. Performance

- **[x] Single-pass JSON envelope.** `to_loco_error()` for `BadRequest` built a
  `serde_json::Value` tree with `json!` and then re-serialized it (two passes,
  several intermediate allocations). Now the code serializes `self` once,
  escapes the message once, and splices the envelope with `format!` — no
  `Value` tree, no `Value::to_string` re-walk.
- **[x] Zero runtime key construction.** Error keys are `&'static str`
  literals baked into the binary by the macro (`error_key()` exposes them),
  instead of being `format!`-ed at runtime.
- **[x] `error_params()` is generated per variant** with `with_capacity`-free
  small `Vec` construction only for variants that actually have fields; unit
  variants return `Vec::new()` (no allocation).
- **[x] `ErrorKind` is `Clone + Copy`-adjacent**: it is `Clone + PartialEq + Eq
  + Hash` and returned by value; no `Rc`/`Box` anywhere on the hot path.
- **[x] `release` profile tuned** (`lto = "thin"`, `codegen-units = 1`).
- **[ ] `Cow<'static, str>` messages** — deferred: `Display` must render
  through `rust_i18n::t!` which returns `Cow<'static, str>` already; wrapping
  it further adds complexity with no measurable win on error paths.

## 3. Modularity

- **[x] Proc-macro crate split into focused modules**: `case` (identifier
  casing), `attrs` (attribute model + parsing), `kind` (kind resolution),
  `expand` (pure `&DeriveInput -> TokenStream` codegen), and a `lib.rs` that
  only wires the entry point. Every module is unit-testable without rustc.
- **[x] Runtime crate split**: `kind.rs` (self-owned `ErrorKind` with
  `Display`/`Serialize`/status mapping) and `lib.rs` (trait + re-exports).
- **[x] No glob imports** (`use loco_rs::prelude::*` removed) — explicit
  imports make the dependency surface visible.
- **[x] Workspace dependency + lint inheritance** — single source of truth for
  versions and the zero-warning policy.

## 4. DRY

- **[x] One canonical kind resolution** (`kind::resolve_kind`) used by codegen,
  instead of a stringly-typed `match` duplicated between name parsing and
  codegen.
- **[x] One `KnownKind` table** drives kebab names, i18n key prefixes,
  `ErrorKind` variants and status codes — adding a kind is a one-line change.
- **[x] Default trait methods** (`status_code`, `to_json`, `to_json_string`,
  `to_loco_error`) are implemented once on top of the three generated
  primitives (`error_kind`, `error_variant`, `error_key`, `error_params`).
- **[x] Hand-rolled `to_kebab_case`** replaces the `convert_case` dependency
  (which dropped its `ccase!` macro in 0.10+ anyway): ~30 lines, unit-tested,
  one less transitive tree.

## 5. KISS

- **[x] Two crates only** (runtime + proc-macro shim), the shim's `lib.rs` is
  ~10 lines.
- **[x] No `serde_json::Value` envelope struct** — a `format!` with two
  pre-escaped strings is easier to audit than a derive-macro envelope type.
- **[x] Deterministic, linear attribute model** instead of a mini-DSL.
- **[ ] no_std support** — deferred: the crate targets loco/axum servers.

## 6. Complementary features (new capabilities)

- **[x] Helper attribute `#[localized(...)]`** with, at enum level:
  - `kind = "bad-request" | "unauthorized" | "not-found" |
    "internal-server-error"` — explicit kind override (no more naming your
    enum by convention);
  - `status = 418` — map the whole enum to a custom HTTP status
    (`ErrorKind::Custom`);
  - `description = "..."` — description carried into Loco's
    `Error::CustomError`;
  - `locale = "fr"` — pin the translation locale;
  - `no_into_response` — opt out of the generated `IntoResponse` impl.
  And, at variant level: `key = "..."`, `message = "..."` (static message,
  bypasses i18n), `status = NNN`, `description`, `locale`.
- **[x] Generated `IntoResponse`** (opt-out via `no_into_response`): error
  enums can be returned straight from axum/loco handlers, rendering the
  structured JSON payload with the right status code.
- **[x] Structured JSON payloads**: `to_json()` /
  `to_json_string()` produce `{kind, variant, key, message, params}` —
  machine-readable errors for SPA/mobile frontends.
- **[x] Introspection methods on the trait**: `error_variant()`,
  `error_key()`, `error_params()` (typed, JSON-ready), `status_code()`.
- **[x] `rust_i18n` re-export** (`locolized_errors::rust_i18n`) and
  generated code paths through it — downstream crates no longer need
  `rust-i18n` or `convert_case` as direct dependencies.
- **[x] `loco_rs` re-export** for the generated `IntoResponse` path.
- **[x] `ErrorKind` is a first-class type**: `Display`, `Serialize` (kebab),
  `status_code()`, `From<ErrorKind> for StatusCode`.
- **[x] Doctests as executable documentation** — the crate-level example
  compiles and runs against the committed `locales/` directory.
- **[x] `trybuild` compile-fail tests** asserting the *text* of macro
  diagnostics.
- **[ ] Macros for `Result` aliasing / `#[handler]` sugar** — deferred:
  overlaps with loco's own ergonomics.
- **[ ] Free-form custom `Backend` injection (i18n backend other than
  rust-i18n)** — deferred until someone asks; rust-i18n covers the 99% case.

## 7. Engineering infrastructure

- **[x] Workspace lints: pedantic clippy + no `unwrap`/`expect`/`panic`/`dbg!`/
  `todo!`/`println!`, `unsafe_code = "forbid"`, `missing_docs`, zero-warning
  policy enforced by CI (`-D warnings`).
- **[x] Test coverage driven to 100% of real code** with `cargo-llvm-cov`:
  unit tests for every macro module, integration tests for every generated
  impl and attribute combination, doctests, trybuild diagnostics. Every
  line, branch and function is executed (0 uncovered rows in the lcov and
  HTML reports); llvm-cov's summary table reports ~99.7% only because it
  charges duplicate generic instantiations (the lib is recompiled per test
  target) against uncovered copies of functions that other targets fully
  exercise.
- **[x] GitHub Actions** (latest action majors): `ci.yml` (fmt, clippy, test,
  doc, MSRV), `coverage.yml` (llvm-cov → Codecov), `release.yml` (git-cliff
  changelog → GitHub Release → crates.io publish), `prepare-release.yml`
  (one-click version bump), Dependabot for Cargo + Actions.
- **[x] `cliff.toml`** — conventional-commits changelog with grouped sections
  and commit links.
- **[x] `LICENSE`** file (crate declared MIT without shipping one).
- **[x] Honest, doctest-verified README** with the real interpolation syntax,
  MSRV and attribute reference.

## 8. Performance iteration (0.3.0)

- **[x] Single-pass JSON payloads.** `to_json` / `to_json_string` /
  `to_json_bytes` now serialize straight from the variant fields through a
  generated `Serialize` payload struct (`__Payload` + `__Params`), replacing
  the `Value`-tree build + `to_string` re-walk. Measured: `to_json_string`
  3.2× faster, `into_response` −34 %.
- **[x] `application/json` for the generated `IntoResponse`.** 0.2.0 returned
  `(status, String)`, and axum serves `String` bodies as `text/plain` — the
  JSON payload was mislabeled. Now built via `__response_from_json` with
  `Body::from(Vec<u8>)` (zero re-copy) and the explicit header.
- **[x] Zero-allocation `ErrorKind` for custom statuses.**
  `Custom(StatusCode, Cow<'static, str>)`; the derive emits
  `Cow::Borrowed("...")`. `error_kind()` dropped from 11.5 ns (allocating) to
  0.81 ns.
- **[x] `error_message()` trait method** — the canonical localized message as
  `Cow<'static, str>`; `Display`, payloads and `to_loco_error` all share it,
  so borrowed translations never force a `String`.
- **[x] Eager fallback allocations removed** from `bad_request_envelope`
  (`unwrap_or` → `unwrap_or_else`).
- **[x] Dependency diet.** `loco-rs` without `with-db` (the crate only uses
  non-gated modules — verified in loco-rs 1.2 sources), `syn` without
  `full`/`extra-traits`, `serde` without `derive` in the runtime crate.
  402 → 344 packages, clean build 137 s → 110 s.
- **[x] Five more named kinds** (`forbidden`, `conflict`,
  `unprocessable-entity`, `too-many-requests`, `service-unavailable`) —
  semantic JSON `kind` values and `errors.<kind>.*` key prefixes instead of
  the opaque `custom`.
- **[x] Criterion benchmark suite** (`benches/perf.rs`) running the 0.2.0
  implementations verbatim against the current API on identical inputs.
- **[x] Codegen hygiene**: all generated items live inside a
  `const _: () = { … }` block, so the two helper structs are scoped per
  derive and cannot collide across enums in one module.
- **[x] 100 % line coverage kept** (cargo-llvm-cov, 0 uncovered lcov rows)
  across the new paths, including the degraded fallbacks for unserializable
  fields and hand-written implementors.
- **[ ] Bypassing `rust_i18n::t!` internals** — deferred: 4.2.4 already uses
  a single-pass state machine and borrowed lookups; calling the
  `#[doc(hidden)]` backend directly would couple us to private API.
- **[ ] no_std / embedded** — still out of scope (axum/loco are std-bound).
