# Contributing to Mojave

Thanks for your interest in contributing! 🎉
This document outlines the process and guidelines to help you contribute effectively. If you have any questions, feel free to reach out via [Telegram](https://t.me/mojavezk) or [Discord](https://discord.gg/wR4srtyhuU).

## Getting Started

1. Fork the repository and clone your fork locally.
2. Set up your development environment:
   ```bash
   git clone https://github.com/1sixtech/mojave.git
   cd mojave
   cargo build
   ```
3. Run the test suite to make sure everything works:
   ```bash
   cargo test
   ```

## Contribution Guidelines

### Issues

- Use [GitHub Issues](https://github.com/1sixtech/mojave/issues) to report bugs, or suggest features.
- Before creating a new issue, search existing ones to avoid duplicates.
- Provide clear steps to reproduce bugs, expected vs actual behavior, and environment info.

### Pull Requests

- Keep PRs focused — one change per PR is ideal.
- The PR title should follow [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format:
  - `fix: ...` for bug fixes
  - `feat: ...` for new features
  - `chore: ...` for maintenance tasks
  - `refactor: ...` for code improvements without changing functionality
  - `test: ...` for adding or updating tests
  - `docs: ...` for documentation changes
  - `perf: ...` for performance improvements
- Add tests for new functionality or bug fixes.
- Ensure `cargo test` and `cargo fmt -- --check` pass before submitting.

### Code Style

- Follow Rust API Guidelines.
- Run `cargo fmt` and `cargo clippy` before pushing.
- Prefer small, composable functions and explicit error handling.

### Development Workflow

- Create a feature branch:
  ```bash
  git checkout -b feature/my-new-feature
  ```
- Commit your changes.
- Push to your fork:
  ```bash
  git push origin feature/my-new-feature
  ```
- Open a Pull Request against main.

## Communication

- Use our communication channels for open-ended ideas.
- For major features, open an issue first to align before coding.
- Be respectful and constructive in discussions.

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.
