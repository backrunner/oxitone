# Contributing to Oxitone

Thanks for helping build Oxitone. The project is pre-1.0 and changes to the TypeScript contracts, Rust engine,
project format and plugin ABI should be discussed in an issue before implementation.

## Development setup

- macOS 13 or newer (Apple Silicon is the primary target; Intel is covered by CI)
- Node.js 24+ and the pnpm version declared in `package.json`
- Rust stable with `rustfmt` and Xcode Command Line Tools

Install dependencies with `pnpm install --frozen-lockfile`. Keep TypeScript authoring code in `packages/*/src` and
native implementation in `crates/*`; generated bindings in `packages/native-generated` are never edited by hand.
Realtime callbacks must remain allocation-free, lock-free, non-blocking and free of JavaScript/N-API calls.

## Before opening a pull request

Run the checks that apply to your change:

```sh
pnpm lint
pnpm typecheck
cargo fmt --all --check
cargo test --workspace
```

Audio tests must use offline PCM/WAV fixtures, simulated native sinks or a browser no-device sink. Never route test
audio to system speakers. Contract or behavior changes also update the relevant specification in `.agents/docs/`.

Use a conventional commit such as `feat(core): add pattern transform`. Pull requests should explain the behavior
change, include focused tests and call out any realtime or file-format implications.

## License and contributions

Oxitone is distributed under the [Mozilla Public License 2.0](LICENSE). Unless a contribution is explicitly marked
otherwise, a contribution submitted for inclusion is provided under the same license.
