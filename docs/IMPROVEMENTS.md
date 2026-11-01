# Brainstorm: Improvements for `locolized-errors`

Status legend: **[x]** implemented in this iteration · **[ ]** deliberately deferred (with reason).

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
