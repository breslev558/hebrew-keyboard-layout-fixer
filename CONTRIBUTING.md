# Contributing

Thank you for helping. Keyboard Layout Fixer is a small Windows 10/11 tray utility written in Rust
(MIT licence). Contributions in English, French or Hebrew are welcome.

## Reporting a problem or asking for a feature
- **Bugs and feature requests:** open an [issue](https://github.com/breslev558/hebrew-keyboard-layout-fixer/issues).
  Say which Windows version, keyboard layouts and program you were typing in, and what you expected.
  You can paste the diagnostic (Settings › Diagnostic › Copy): it contains program names and steps,
  never your text. Never paste passwords or personal text.
- **Security problems:** do **not** open a public issue; follow [SECURITY.md](SECURITY.md).
- Please follow the [code of conduct](CODE_OF_CONDUCT.md).
- Issues get a first answer within 14 days. Closed issues stay public as an archive.

## Changing the code
1. Fork the repository and create a branch.
2. Build and test (see *Build from source* in the [README](README.md)):
   ```
   rustup target add x86_64-pc-windows-gnu
   bash tools/construire.sh
   ```
3. Open a pull request that explains what changes for the user and why.

## Requirements for a pull request
- **Tests:** every new feature or bug fix in the conversion core (`crates/klf-core`) comes with
  automated tests (`cargo test -p klf-core`). Windows-only behaviour gets a manual check listed in
  [TESTS.md](TESTS.md) and, for installer scripts, a PowerShell test in `tests-ps/`.
- **No warnings:** `cargo clippy --all-targets -- -D warnings` must pass for both crates (the
  Windows crate with `--target x86_64-pc-windows-gnu`); format with `cargo fmt`.
- **Dependencies:** as few as possible, permissive licences only, checked by `cargo deny check`
  and `cargo audit`. No new network access, telemetry or logging of typed text.
- **Privacy:** the program must never send, store or log what the user types or copies.
- The automatic checks on GitHub (tests, clippy, cargo-audit, cargo-deny, fuzzing, CodeQL, Windows
  installer test) must be green before a change is merged.

## Releases
Versions follow [semantic versioning](https://semver.org) (`MAJOR.MINOR.PATCH`) and are tagged
`vX.Y.Z`. Every release has a [CHANGELOG.md](CHANGELOG.md) entry written for users, which names any
security problem fixed. Release files are built by `.github/workflows/release.yml` with a build
provenance attestation and SHA-256 checksums.
