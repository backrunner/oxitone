# Oxitone development rules

Oxitone is a TypeScript authoring SDK backed by a Rust realtime audio engine. The project is macOS-first,
distributed through npm, and must keep the audio callback independent from JavaScript.

Before changing code, read the applicable document in `.agents/docs/` and apply the local skill at
`.agents/skills/oxitone-guard/SKILL.md`.

## Repository rules

- Use `pnpm` for the TypeScript workspace and `cargo` for Rust crates. Keep package and crate names aligned
  with the published package map in `.agents/docs/01-architecture.md`.
- Keep public TS contracts in `packages/*/src` and native implementation in `crates/*`. Generated bindings
  belong in `packages/native-generated` and are never hand-edited.
- Use `apply_patch` for manual edits. Do not commit build output (`target`, `dist`, native binaries, or
  `node_modules`) unless a release task explicitly requests an artifact.
- Use conventional commits: `xxx(comp): desc` with a lowercase type and scope.
- Required checks for implementation changes are `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check`,
  `cargo test --workspace`, and the focused benchmark. Narrow docs-only changes may skip code checks.

## Architecture invariants

- TypeScript builds a declarative project and sends versioned commands/snapshots to Rust.
- Rust owns the compiled graph, scheduling, synthesis, sample decoding, effects, mixing, CoreAudio I/O,
  offline WAV rendering, and MIDI export.
- Realtime code is allocation-free, lock-free, non-blocking, and free of N-API calls.
- Any contract or behavior change must update the relevant `.agents/docs/` specification and tests.
