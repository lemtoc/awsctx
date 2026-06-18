# Contributing

Thanks for helping improve `awsctx`.

This project is a small Rust CLI for switching AWS SSO profiles in the current shell. Keep changes focused, predictable, and easy to verify locally.

## Development Setup

Use the Nix development shell:

```sh
nix develop
```

If you use direnv:

```sh
direnv allow
```

The development shell provides Rust, rustfmt, clippy, cargo-dist, and related tooling.

## Checks

Run these before sending changes:

```sh
cargo fmt --check
cargo check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

## Local Smoke Tests

Build the debug binary:

```sh
cargo build
export PATH="$PWD/target/debug:$PATH"
```

Load shell integration without editing your shell config:

```sh
eval "$(awsctx activate zsh)"
```

For bash:

```sh
eval "$(awsctx activate bash)"
```

For fish:

```fish
awsctx activate fish | source
```

Then try:

```sh
awsctx list
awsctx
echo "$AWS_PROFILE"
```

## Documentation

User-facing documentation lives in `README.md`.

Project planning and domain notes live under `.docs/`:

- `.docs/CONTEXT.md`
- `.docs/REQUIREMENTS.md`

Architecture decisions live under `docs/adr/`.

## Release Process

Releases are generated with cargo-dist.

Push a SemVer tag such as `v0.1.0` to run the release workflow. The workflow builds GitHub Release artifacts and a Homebrew formula.

To publish the formula to `lemtoc/homebrew-tap`, set the `HOMEBREW_TAP_TOKEN` GitHub Actions secret in `lemtoc/awsctx`. The token must be allowed to push to `lemtoc/homebrew-tap`.
