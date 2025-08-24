# Project Structure

This document defines the **layout and conventions** for the Mojave repository.

---

## 1) Top-level Layout

```bash
.github/workflows/ # CI
cmd/ # all runnable binaries live here
crates/ # reusable libraries (no top-level binaries)
data/ # chain and configs data (e.g., genesis.json)
test_data/ # e2e fixtures and scripts
scripts/ # (optional) repo scripts
justfile # common dev tasks
Cargo.toml # workspace manifest
rust-toolchain.toml # toolchain pin
rustfmt.toml # formatting rules
.env.example # environment template
```

## 2) Binaries in `cmd/`

**Standards**

- `main.rs` only wires tracing, loads config, parses CLI, and calls `run()`.
- `cli.rs` uses **clap** (derive) and must provide:
  - `Cli` (top-level args), optional subcommands via `enum`.
  - `impl Cli { pub fn run(self) -> Self { ... } }`
- `*.rs` for additional modules (e.g., config loading).
- No business logic in `cmd/*`; logic belongs to `crates/*`.

**Example `main.rs`**

```rust
fn main() -> anyhow::Result<()> {
  mojave_utils::logging::init(); // workspace-standard tracing init
  let cli = cli::Cli::run();

  ... // load config, setup context, run app
}
```

## 3) Library Crates in crates/

**File Structure**

```bash
src/
	rpc/ # optional rpc surface
	lib.rs # exports + prelude
	error.rs # thiserror types
	types.rs # domain types & serde-facing structs
	utils.rs # optional helpers specific to this crate
	*.rs # core logic
```

**Example lib.rs**

```rust
pub mod error;
pub mod types;
// pub mod <feature_area>;
pub mod prelude {
	pub use crate::types::\*;
	pub use crate::error::{Error, Result};
}
```

## 4) RPC Modules (when applicable)

```
rpc/
	mod.rs
	block.rs
	transaction.rs
	types.rs
```

Rules:

- Keep transport-agnostic business logic outside `rpc/*`.
- `rpc/types.rs` only contains request/response DTOs (serde).

## 5) Modules

- One file per concern: error.rs, types.rs, context.rs, service.rs.
- Use mod.rs only for directory root.
- Public surface: re-export types in lib.rs or prelude to keep imports stable.
- Async: prefer tokio traits and async fn; avoid blocking calls.

## 6) Errors

- `thiserror` for domain errors: `pub enum Error`.
- `anyhow::Result<T>` for bin/CLI boundaries; `crate::error::Result<T>` internally.
- Wrap external errors with context (anyhow::Context) only at boundaries.

  ```rust
  pub type Result<T> = std::result::Result<T, Error>;

  #[derive(thiserror::Error, Debug)]
  pub enum Error {
  	#[error("rpc failed: {0}")]
  	Rpc(#[from] rpc::Error),
  	#[error("config invalid: {0}")]
  	Config(String),
  }
  ```

## 7) Types

- `types.rs`: pure data (serde-ready), no side effects.
- Use `#[serde(deny_unknown_fields)]` where inputs come from untrusted sources.
- Prefer type wrappers for IDs/Keys; avoid naked String/Vec<u8>.

## 8) Configuration

- Source order: CLI > Config file > env (.env) > defaults (in code).
- Validate early; fail-fast with actionable messages.

## 9) Logging & Metrics

- `crates/utils/src/logging.rs` provides tracing.
- Respect `RUST_LOG` and/or `--log-level`.
- Leave room for metrics export (e.g., tracing -> OpenTelemetry) behind a feature flag.

## 10) Testing

- Unit tests colocated with code (`#[cfg(test)]`).
- Integration tests in `crates/tests/tests/*.rs`.
- E2E tests and scripts under /tests/ (document required env).

Commands for running tests:

```bash
cargo test --workspace
bash test_data/tests-e2e.sh
```

## 11) Features & Dependency Policy

- Default feature set: minimal, deterministic.
- Additive features only; avoid mutually exclusive flags when possible.
- External deps must pin major versions and avoid enabling heavy default features.
- Patches go in root [patch.crates-io] with comments and tracking issue.

## 12) Versioning & Releases

- Crates start at 0.1.0.
- Bump minor for additive changes, patch for fixes.
- Tag workspace releases vX.Y.Z.

## 13) CI & Lint

GitHub Actions must enforce:

```bash
cargo build --workspace --locked
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
# PR title lint (conventional commits)
```

## 14) Security

- No unsafe code without justification and // SAFETY: comments.
- Avoid unchecked unwrap()/expect() outside tests.

## 15) Rules of Thumb

### Do

- Keep binaries thin, libraries fat.
- Reuse utils, client, etc. instead of duplicating code ([Rule of three](<https://en.wikipedia.org/wiki/Rule_of_three_(computer_programming)>)).
- Document public functions with examples.
- Add tests for critical paths and edge cases.
- Document non-obvious decisions in code comments.

### Don’t

- Reach across crates; depend on public APIs only.
- Put config/env parsing in libraries.
- Introduce new global singletons; prefer explicit handles/contexts.
