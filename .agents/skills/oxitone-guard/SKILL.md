---
name: oxitone-guard
description: Review and implement Oxitone changes within TypeScript, Rust, and realtime audio boundaries.
---

# Oxitone Guard

Use this skill for any Oxitone code, API, DSP, plugin, project-format, packaging, or benchmark change.

## Required boundaries

- Keep TypeScript as the authoring/control API. Rust owns decoding, scheduling, DSP, mixing, device I/O,
  realtime state, and offline rendering. Native callbacks run no TypeScript/JavaScript; the Web Audio
  worklet is a bounded PCM-copy adapter only (see 12-wasm-web-audio.md), with Rust processing in a Worker.
- Cross the boundary through a versioned, typed N-API or Wasm memory facade. Do not expose Rust structs directly as a
  mutable TypeScript object graph; snapshots and commands must be explicit and serializable.
- Treat the audio callback as hard realtime: no allocation, locks, blocking I/O, filesystem/network access,
  logging, JSON, promises, or N-API calls. Use preallocated buffers and lock-free queues.
- Keep one-way ownership: Project/Track/Pattern/Automation are authoring data; the Rust engine owns the
  compiled render graph. A render graph is rebuilt off-thread and swapped at a block boundary.
- Use beats/bars for authoring and integer sample frames for the DSP timeline. Convert at the transport
  boundary; never mix seconds and beats in one field.
- Reject invalid graphs early: duplicate IDs, dangling references, negative durations, invalid automation
  ranges, mixer cycles, sidechain cycles, unsupported sample formats, and parameter IDs unknown to a node.
- Keep source files cohesive. Split files before they exceed roughly 300 lines or contain more than one
  domain responsibility. Avoid `utils` dumping grounds and boolean-option APIs that hide incompatible modes.
- Public APIs require docs, stable error codes, and focused tests. Any realtime-path change requires a
  benchmark or an explanation in the change record.
- Test audio must never reach system output devices or speakers. Native realtime tests use the simulated
  sink; browser tests require a no-device AudioContext sink and fail closed if unsupported.

## Change workflow

1. Read the relevant `.agents/docs/*` specification before editing. Update the spec when the contract changes.
2. Identify the layer: `packages/*` for TypeScript authoring/API, `crates/*` for Rust runtime/DSP, and
   `packages/native-*` only for thin platform packaging.
3. Define or update the versioned TS contract first, then implement Rust validation and execution.
   The project is unpublished; the approved source/DAW redesign (`../../designs/source-daw/README.md`)
   permits breaking changes. Migrate callers, tests and documentation together and delete superseded
   implementations and compatibility shims. Reject unsupported versions without rewriting or deleting
   recovery data; do not relabel old payloads as a new format.
4. Add tests at the narrowest useful layer: pure TS model tests, Rust DSP tests, graph/transport tests,
   and an integration test through the native facade where the boundary is affected.
5. Run formatting, type checking, Rust tests, and the focused benchmark. Record device, sample rate,
   block size, CPU, p95/p99 callback time, and xrun count for performance-sensitive changes.

## Review checklist

- Is the change deterministic for the same project, seed, and render settings?
- Can it be used in offline render without a realtime device?
- Does it preserve sample-accurate event ordering at bar and loop boundaries?
- Does it respect the render-ahead horizon, underrun policy, and denormal safety (FTZ/DAZ)?
- Does it preserve deterministic summation order, PDC alignment, and export dither/loudness rules?
- Does it introduce work, allocation, or locking in the callback?
- Are automation, sidechain, tail, channel layout, and error cases covered?
- Are package exports, generated schemas, docs, and benchmarks updated together?

For domain details, read `../../docs/01-architecture.md`, `../../docs/02-domain-spec.md`,
`../../docs/03-audio-runtime-spec.md`, `../../docs/04-api-contracts.md`, and
`../../docs/07-automation-spec.md` as needed. Any automation source or evaluator change must read
`07-automation-spec.md` and update its golden vectors when observable output changes. Any plugin
loading, ABI, manifest, or third-party distribution change must read `08-plugin-abi.md` and keep
the conformance fixtures in sync. Preview/viewer changes must read `09-preview-app.md` and keep the
UI writes behind the Node Document Service's semantic transactions. The approved source/DAW design
supersedes the former read-only product restriction; the currently shipped viewer remains read-only
until its edit protocol and accepted-revision projection are implemented. No GUI writes Rust state
or source files directly. Never add entity IDs, UUID comments, or binding decorators to user source.
