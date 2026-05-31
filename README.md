# cfgdrift

[![CI](https://github.com/vlad-ds/cfgdrift/actions/workflows/ci.yml/badge.svg)](https://github.com/vlad-ds/cfgdrift/actions/workflows/ci.yml)

`cfgdrift` is a fast Rust CLI for finding semantic drift between config files or config directories. It understands JSON, YAML, TOML, and `.env`, redacts secret-looking values by default, and emits human, JSON, or Markdown output for local review and CI.

Plain `diff` is noisy for config review because it sees formatting, key order, and quote changes. Format-specific tools help, but teams usually keep mixed config trees. `cfgdrift` compares the parsed settings and reports the field paths that actually changed.

## Install

```sh
cargo install --git https://github.com/vlad-ds/cfgdrift
```

Or run from a checkout:

```sh
cargo run -- diff examples/baseline examples/candidate
```

## Quick Start

```sh
cfgdrift diff config/prod config/staging
```

Example output:

```text
cfgdrift found 5 drifts between examples/baseline and examples/candidate

~ app.toml:database.password
  baseline:  <redacted>
  candidate: <redacted>

~ app.toml:database.pool
  baseline:  8
  candidate: 16

~ app.toml:features.checkout_v2
  baseline:  false
  candidate: true

~ settings.env:PUBLIC_URL
  baseline:  "https://example.com"
  candidate: "https://staging.example.com"

~ settings.env:STRIPE_SECRET_KEY
  baseline:  <redacted>
  candidate: <redacted>
```

Use `--format markdown` for PR comments:

```sh
cfgdrift diff config/prod config/staging --format markdown
```

Use `--format json` for automation:

```sh
cfgdrift diff config/prod config/staging --format json --exit-zero
```

## Why This Exists

Config drift is easy to miss and hard to review cleanly. Production, staging, CI, and local config often live in different files and formats, while normal text diffs mix real setting changes with formatting, ordering, and quoting noise.

`cfgdrift` focuses on the parsed configuration values and reports stable field-level changes that are useful in terminals, pull requests, and automation.

## Supported Inputs

- JSON: `.json`
- YAML: `.yaml`, `.yml`
- TOML: `.toml`
- dotenv: `.env`, `.env.*`, `.env` extension

When comparing directories, unsupported files are ignored. `cfgdrift` skips `.git`, `target`, and `node_modules`.

## Ignore Rules

Ignore a path directly:

```sh
cfgdrift diff prod staging --ignore database.pool
```

Use an ignore file:

```sh
cfgdrift diff prod staging --ignore-file .cfgdriftignore
```

`.cfgdriftignore` supports blank lines, `#` comments, direct suffix matches, and glob patterns:

```gitignore
# noisy deploy metadata
build.generated_at
*.yaml:metadata.revision
secrets.*
```

## Exit Codes

- `0`: no drift, or drift was found with `--exit-zero`
- `1`: drift found
- `2`: input, parse, or runtime error

## Commands

```sh
cfgdrift diff <baseline> <candidate> [OPTIONS]
cfgdrift formats
```

Useful flags:

- `--format human|json|markdown`
- `--ignore <PATTERN>`
- `--ignore-file <PATH>`
- `--summary-only`
- `--no-redact`
- `--exit-zero`

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## License

MIT
