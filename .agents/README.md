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
- `docs/09-preview-app.md` - current GPUI preview, source/DAW migration boundaries, and sync semantics.
- `docs/10-implementation-status.md` - implementation evidence, remaining milestone gaps, and next steps.
- `docs/15-source-authoring.md` - implemented Pattern source graph, edits, and initial TS expression writer.
- `docs/16-pattern-document.md` - Pattern source ownership, imports, and reviewed local materialization within ProjectDocument.
- `docs/17-project-source-session.md` - real evaluation, draft overlays, disk synchronization, TS Save and recovery.
- `docs/18-project-daw.md` - full-project note transactions, GPUI scopes/review/conflicts, editor IPC and plugin catalog.
- `docs/19-plugin-configuration-source.md` - immutable configurations, parameter writeback and reviewed npm serial rack localization.
- `docs/20-plugin-instances.md` - engine 1.1 instances, scoped automation and GPUI effect ordering without source IDs.
- `docs/21-editor-session.md` - VS Code linked source buffers, live synchronization, conflict review and journal Save.
- `docs/22-engineering.md` - module boundaries, file size, dependency direction and refactor checks.
- `docs/23-daw-controls.md` - source-backed Mixer/tempo, clip operations and plugin assignment.
- `skills/oxitone-guard/SKILL.md` - mandatory development and review guardrails.

The documents describe the Phase 1 product. VST3 is a Phase 2 adapter and must not leak into Phase 1 core
interfaces.

## Target architecture: code / DAW authoring

The project is unpublished; the user has authorized redesigning the authoring contracts, preview role,
protocols, and plugin ABI. [Code / DAW architecture](designs/source-daw/README.md) is the consolidated
target design for that work, including source writing, transactions, runtime semantics, external plugins,
the GPUI plugin manager, and delivery gates. It supersedes the design recommendations in the proposals below.
The migration notices and guardrails now distinguish the target from the existing engine/ABI baseline.
The first implemented slice is recorded in [source authoring](docs/15-source-authoring.md); it does not
complete the full P0–P5 release gates.
The [2026-09-09 implementation review](reports/2026-09-09-source-daw-review.md) records reproduced fixes,
integration evidence, and the outstanding requirements across the full delivery matrix.

## Historical discussion proposals

These proposals preserve the investigation and source audit that informed the consolidated target design.

- `proposals/2026-09-08-source-backed-daw.md` - TS source-backed GPUI editing without explicit author IDs,
  including higher-order generation, instance exceptions, local expansion, and save/conflict semantics.
- `proposals/2026-09-08-higher-order-edit-audit.md` - current chord/arp, all 24 automation methods,
  clip/sample transforms, generated slicing/zones, and configuration writeback rules and limitations.
- `proposals/2026-09-08-plugin-manager.md` - external-plugin compatibility, GPUI plugin management,
  source-backed registration and instance editing, dependency/version tasks, and ABI capability gaps.
