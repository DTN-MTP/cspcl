# Contributing to CSPCL

Thank you for your interest in contributing! This document explains how to get started.

## Getting Started

1. **Fork** the repository and clone your fork.
2. Create a feature branch: `git checkout -b feat/your-feature`
3. Build and run the tests to confirm a clean baseline (see [Getting Started](https://dtn-mtp.github.io/cspcl/getting-started/)).

## Development Setup

```bash
git clone https://github.com/DTN-MTP/cspcl.git
cd cspcl && mkdir build && cd build
cmake -DCSP_REPO_DIR=/path/to/libcsp ..
make
ctest --verbose
```

## Coding Standards

- **C**: Follow the existing style (C11, clang-format); run `clang-format -i src/*.c src/*.h` before committing.
- **Rust**: `cargo fmt` and `cargo clippy` must pass with no warnings.
- Keep public API changes minimal and backward-compatible within the same major version.
- All new functionality must be accompanied by tests.

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat(cspcl): add connection pool timeout option
fix(cspcl): prevent double-free on init failure
docs: update getting-started guide
```

## Submitting a Pull Request

1. Make sure `ctest` passes with the stub build.
2. Update `CHANGELOG.md` under `[Unreleased]`.
3. Open a PR against the `main` branch.
4. Fill in the pull request template.

## Reporting Bugs

Use the [bug report](.github/ISSUE_TEMPLATE/bug_report.yml) template. Include a minimal reproducer and the output of `cmake --version` and your libcsp version.

## License

By contributing you agree that your contributions will be licensed under the [MIT License](LICENSE).
