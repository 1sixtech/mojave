# Copilot Instructions for Mojave

## Project Overview

**Mojave** is a new layer built on top of Bitcoin. It brings scalability, programmability, and fast transaction speed without compromising Bitcoin's core strengths: security and decentralization.

This is a Rust-based blockchain infrastructure project organized as a Cargo workspace with multiple binaries (node, sequencer, prover) and supporting libraries.

## Tech Stack

- **Language**: Rust (nightly toolchain)
- **Build System**: Cargo workspace
- **Task Runner**: `just` (justfile)
- **Core Dependencies**:
  - ethrex (custom fork for blockchain/EVM functionality)
  - Bitcoin libraries (bitcoin, bitcoincore-rpc)
  - Async runtime: tokio
  - RPC: axum
  - Serialization: serde, bincode
  - Cryptography: secp256k1, ed25519-dalek
  - Messaging: zeromq
  - Error handling: thiserror, anyhow

## Project Structure

The repository follows a strict structure:

```
.github/workflows/    # CI/CD workflows
cmd/                  # All runnable binaries (node, sequencer, prover)
crates/               # Reusable libraries (no top-level binaries)
data/                 # Chain configs and genesis files
tests/                # E2E test fixtures and scripts
scripts/              # Utility scripts
justfile              # Common development tasks
```

### Key Principles

- **Keep binaries thin, libraries fat**: Business logic belongs in `crates/`, not in `cmd/`
- **Each binary in `cmd/`**: Has `main.rs` (minimal), `cli.rs` (clap-based), and delegates to library crates
- **Library structure**: Each crate has `lib.rs`, `error.rs`, `types.rs`, and domain modules
- **No unsafe code** without justification and `// SAFETY:` comments
- **Explicit error handling**: Use `thiserror` for domain errors, `anyhow::Result` only at binary boundaries

## Coding Standards

### Code Style

- **Format**: Always run `cargo fmt` before committing (uses `rustfmt.toml` config)
- **Imports**: Use `imports_granularity = "Crate"` (configured in rustfmt.toml)
- **Linting**: Run `cargo clippy --all-targets --all-features --workspace -- -D warnings`
- **Comments**: Add comments only when they match existing style or explain complex logic
- **Naming**: Follow Rust API Guidelines
- **Functions**: Prefer small, composable functions

### Error Handling

```rust
// In libraries: use thiserror
pub type Result<T> = core::result::Result<T, Error>;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("rpc failed: {0}")]
    Rpc(#[from] rpc::Error),
    #[error("config invalid: {0}")]
    Config(String),
}

// In binaries: use anyhow::Result<T> at boundaries only
```

### Types and Serialization

- Define types in `types.rs` with serde support
- Use `#[serde(deny_unknown_fields)]` for untrusted input
- Prefer type wrappers for IDs/Keys over naked `String`/`Vec<u8>`

### Module Organization

- One file per concern: `error.rs`, `types.rs`, `context.rs`, `service.rs`
- Use `mod.rs` only for directory roots
- Re-export public APIs in `lib.rs` or `prelude` module

## Building and Testing

### Commands

```bash
# Build the project
cargo build
cargo build --release

# Run tests
cargo test --workspace

# Run E2E tests
bash tests/tests-e2e.sh

# Format code
cargo fmt --all

# Lint code
cargo clippy --all-targets --all-features --workspace -- -D warnings

# Using justfile
just build        # Build in debug mode
just lint         # Format check + clippy
just test         # Run E2E tests
just fix          # Auto-fix issues (fmt, clippy, etc.)
```

### Test Organization

- Unit tests: Colocated with code using `#[cfg(test)]`
- Integration tests: In `crates/tests/tests/*.rs`
- E2E tests: In `/tests/` directory with bash scripts

### CI Requirements

All PRs must pass:
- `cargo build --workspace --locked`
- `cargo test --workspace`
- `cargo fmt --all --check`
- `cargo clippy --workspace --all-features --all-targets --no-deps -- -D warnings`
- PR title must follow Conventional Commits format

## Git Workflow

### Branch Naming

- Feature branches: `feature/<description>`
- Bug fixes: `fix/<description>`
- Copilot branches: `copilot/<description>`

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/) format:
- `feat:` - New features
- `fix:` - Bug fixes
- `chore:` - Maintenance tasks
- `refactor:` - Code improvements without changing functionality
- `test:` - Test additions or updates
- `docs:` - Documentation changes
- `perf:` - Performance improvements

### Pull Request Guidelines

- Keep PRs focused - one change per PR
- PR titles must follow Conventional Commits format
- Add tests for new functionality or bug fixes
- Ensure all CI checks pass before submitting
- Update documentation if relevant

## Boundaries and Security

### What Copilot SHOULD Do

- Implement features according to the project structure guidelines
- Add unit and integration tests for new code
- Follow existing code patterns and conventions
- Update documentation when making changes
- Use workspace dependencies defined in root `Cargo.toml`
- Apply appropriate error handling patterns

### What Copilot SHOULD NOT Do

- **Never commit secrets, private keys, or credentials**
- **Never modify `.env` files** (only `.env.example` is acceptable)
- **Never introduce unsafe code** without explicit justification
- **Never use `unwrap()` or `expect()` outside of tests**
- **Never add new external dependencies** without checking if a workspace dependency exists
- **Never modify the core project structure** (cmd/, crates/ layout)
- **Never bypass existing error handling patterns**
- **Never modify configuration files** in `/data/` without clear requirements
- **Never add non-additive feature flags** that break existing functionality

## Common Tasks

### Adding a New Binary

1. Create directory in `cmd/<binary-name>/`
2. Add `main.rs` (minimal, just wiring)
3. Add `cli.rs` with clap-based CLI
4. Add to workspace members in root `Cargo.toml`
5. Implement logic in appropriate `crates/` library

### Adding a New Library Crate

1. Create directory in `crates/<crate-name>/`
2. Create `Cargo.toml` with workspace inheritance
3. Add standard structure: `lib.rs`, `error.rs`, `types.rs`
4. Add to workspace members and dependencies in root `Cargo.toml`
5. Document public APIs

### Adding Dependencies

1. Check if the dependency exists in workspace `[workspace.dependencies]`
2. If yes, reference it from your crate's `Cargo.toml`
3. If no, add to workspace dependencies first, then reference
4. Pin major versions and avoid heavy default features
5. Document reason for new dependencies

## Configuration

- **Source order**: CLI args > Config file > Environment (.env) > Code defaults
- **Validation**: Validate configuration early with actionable error messages
- **Environment**: Use `.env.example` as template, never commit `.env`

## Logging

- Use `mojave-utils::logging::init()` for tracing initialization
- Respect `RUST_LOG` environment variable
- Use structured logging with `tracing` crate
- Add context to errors at boundaries using `anyhow::Context`

## Documentation

- Document all public functions with rustdoc comments
- Include examples in documentation where helpful
- Keep `PROJECT_STRUCTURE.md` updated for structural changes
- Update `README.md` for user-facing changes
- Keep `CONTRIBUTING.md` updated for process changes

## Resources

- [LICENSE](../LICENSE) - MIT License
- [CONTRIBUTING.md](../CONTRIBUTING.md) - Contribution guidelines
- [PROJECT_STRUCTURE.md](../PROJECT_STRUCTURE.md) - Detailed project structure
- [README.md](../README.md) - Project overview and quickstart

## Questions?

For questions about Mojave:
- [Telegram](https://t.me/mojavezk)
- [Discord](https://discord.gg/wR4srtyhuU)
- [GitHub Issues](https://github.com/1sixtech/mojave/issues)
