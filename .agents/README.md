# Oxitone agent documents

These documents are the implementation contract for Oxitone. They are intentionally split so an agent can
load only the part needed for a change.

- `docs/00-roadmap.md` - milestones, sequencing, and release gates.
- `docs/01-architecture.md` - workspace layout, ownership, and integration decisions.
- `docs/02-domain-spec.md` - Project, Track, Pattern, Note, Sample, Channel, Mixer, and Automation semantics.
- `docs/03-audio-runtime-spec.md` - realtime DSP, transport, device I/O, rendering, and failure behavior.
- `docs/04-api-contracts.md` - TypeScript interfaces and Rust/N-API boundary contracts.
- `docs/05-performance-and-benchmarks.md` - benchmark protocol, budgets, and regression policy.
- `docs/06-format-and-export.md` - project serialization, sample policy, WAV/MIDI export, and compatibility.
- `docs/07-automation-spec.md` - automation source formulas, high-level functions, validation, and tests.
- `docs/08-plugin-abi.md` - third-party plugin C ABI, dynamic loading, npm distribution, and conformance.
- `docs/09-preview-app.md` - GPUI read-only preview app, runner/viewer process split, and sync semantics.
- `docs/10-implementation-status.md` - implementation evidence, remaining milestone gaps, and next steps.
- `skills/oxitone-guard/SKILL.md` - mandatory development and review guardrails.

The documents describe the Phase 1 product. VST3 is a Phase 2 adapter and must not leak into Phase 1 core
interfaces.
