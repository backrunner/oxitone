# @oxitone/native-generated

Generated N-API bindings for the Oxitone native engine (`crates/napi`,
crate `oxitone-napi`). **Never hand-edit `index.js` / `index.d.ts`** — they
are produced by [napi-rs](https://napi.rs) and regenerated on every build.

## How the files are generated

```sh
pnpm build:native          # from the repo root, or:
pnpm --filter @oxitone/native-generated run build
```

`scripts/build-native.mjs` runs `napi build --platform --release` in
`crates/napi` (binary name `oxitone-native`, configured in
`crates/napi/package.json`) and copies the results into this package:

- `index.js` / `index.d.ts` — the napi-rs generated binding glue and
  declarations (source of truth for the native function signatures),
- `oxitone-native.<platform>-<arch>.node` — the compiled addon
  (e.g. `oxitone-native.darwin-arm64.node`).

`*.node` binaries are build output and are not committed.

## Binary resolution order

The `oxitone` facade package (`packages/native`) loads the `.node` addon
itself (the generated `index.js` is kept for declarations/reference). It
searches in this order:

1. `OXITONE_NATIVE_PATH` — absolute path to a `.node` file (override for
   debugging / CI).
2. This package's directory — `oxitone-native.<platform>-<arch>.node`
   placed here by the build above. (M5 will additionally resolve optional
   platform packages such as `@oxitone/native-darwin-arm64`; the interface
   is stubbed in `packages/native/src/load.ts`.)
3. Development build outputs, relative to the repo root:
   `crates/napi/oxitone-native.<platform>-<arch>.node` (raw napi-cli output),
   then `target/release/liboxitone_napi.dylib` and
   `target/debug/liboxitone_napi.dylib` (plain `cargo build -p oxitone-napi`,
   loaded via `process.dlopen`).
