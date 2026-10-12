# Contributing to Phonetic

Thanks for helping. Phonetic is small; a short issue or a focused pull request is the best way in.

## Before you start

- Open an issue first for anything bigger than a typo or a one-line fix, so we agree on the
  approach before you spend time on it.
- Changes to measurement maths need a reference: name the standard (IEC 61672, ISO 3382, AES17,
  ...) or paper you are implementing in the pull request.
- macOS and Linux only. Windows is out of scope; please do not add Windows code paths.

## Setup

See [Build from source](README.md#build-from-source). `just install` fetches the toolchains and
dependencies, `just dev` runs the app with hot reload.

## Workflow

1. Branch from `main`.
2. Make the change, with tests. DSP changes get a unit test in Rust; `just test-accuracy` runs
   the signal-level checks against known inputs.
3. Run `just pre-commit`. It runs Biome, clippy with warnings denied, rustfmt and both test
   suites, which is what CI runs on macOS and Linux, arm64 and x86_64.
4. Use conventional commit messages: `feat(dsp): ...`, `fix(ui): ...`, `docs: ...`.
5. Open a pull request against `main` and say how you verified it.

## Style

- Rust: rustfmt and clippy decide. Platform-specific code lives behind `cfg(target_os = ...)`,
  with CPAL as the backend that works everywhere.
- TypeScript: Biome decides. Small `const` arrow components, `@/` imports, no classes.
- Keep it plain. No new dependency for what a few lines can do.

## Licence of contributions

By submitting a pull request you agree that your contribution is licensed under the project's
[LICENSE](LICENSE), Apache 2.0 with the Commons Clause, as described in section 5 of the Apache
licence. You keep your copyright.
