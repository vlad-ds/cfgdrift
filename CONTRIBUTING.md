# Contributing

Thanks for helping improve `cfgdrift`.

## Local Checks

Run the same checks as CI before opening a pull request:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Design Principles

- Prefer structural parsing over text heuristics.
- Keep output stable enough for CI and PR comments.
- Redact sensitive values by default.
- Add focused tests for parser, diff, and CLI behavior when changing user-visible output.

## Release Notes

Keep user-visible changes in `CHANGELOG.md`.
