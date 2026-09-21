# VST3 presets and frozen Playlist audio

This increment adds explicit local preset files and source-backed audio placement to the existing offline
workbench. It does not add realtime VST3 instances, transport automation or a vendor editor. No realtime
callback code changed. The previous workbench report describes the preceding increment.

## Contracts and boundaries

- `oxitone/vst3` / `@oxitone/vst3`: `parseVst3Preset`, `saveVst3Preset`, `loadVst3Preset` and `Vst3Preset`.
  Format 1 retains the absolute bundle path, exact class/hash, opaque state and normalized parameters.
  Strict parsing excludes imported trust policy. Files are bounded to 8 MiB; opaque state retains its 4 MiB
  limit. Unknown versions and existing files are preserved. Atomic create-only publication uses an adjacent
  0600 staging file, fsync, hard link and directory fsync. Loading rejects symlinks and non-regular files,
  allocates the observed file size plus a growth sentinel and rejects size changes during the read.
- DAW Files / project saves and loads presets through Document Service. Imported state survives catalog
  refresh; verification does not. Inspect validates class/hash and writable parameter IDs, then Render
  restores opaque state. A preset never relaxes project signature policy. The 24 MiB session state budget
  includes imported presets; a rejected replacement keeps the prior preset.
- `attachRender` verifies the last completed WAV's hash, frames, sample rate and channels through native
  cacheSample. The file must be regular and at most 256 MiB. Project asset directory components cannot be
  symlinks. The source transaction references owned, content-addressed `assets/vst3` WAVs using relative
  URIs, creates a new Track/sample clip through `Project.importAudio`, and verifies the whole candidate.
  Playback has tempoSync off and retains the complete tail and latency. Existing audio and music remain
  subject to the normal candidate/native validation and accepted-revision flow.
- Source Undo/Redo/Save/reopen use the existing journal. Original render files and the VST3 installation are
  unnecessary for the frozen audio. Undo and rejected candidates retain immutable cached assets for
  recovery; automatic orphan cleanup is not implemented. Preset writes are independent external files and
  are not undone by a music Undo. An already published preset is preserved if a later generation check fails.
- Pure authoring preflight validates sample/timing and reserves all three authoring revisions before mutation.
  Source generation emits no fixed entity IDs. The Document operation is one undoable revision.

## Verification

- `pnpm format:check`, `pnpm lint`, `pnpm typecheck`, `cargo fmt --all --check` passed.
- `pnpm test`: **441 passed** across the workspace. `cargo test --workspace`: **554 passed**, three
  intentionally ignored benchmark tests. After final UI copy/Unicode sizing changes, the preview suite
  passed **73 tests**; after sizing preset read buffers, all **12 SDK tests** passed again.
- Focused coverage includes parallel create-only saves, future-version/corrupt-file preservation, forbidden
  trust fields, class mismatch, oversize and symlink rejection, imported-state budget retention, unknown and
  readonly parameters, changed WAV rejection, asset directory containment, authoring revision exhaustion,
  and source Undo/Redo/Save. A fresh ProjectDocument successfully reopens the audio after deleting the original
  WAV and making the helper unavailable. `/var` and `/private/var` aliases are canonicalized before making
  relative asset paths.
- `scripts/smoke-vst3.mjs --gui` passed twice using the installed Vesti Gain fixture, class
  `56455354494741494e30303030303031`, hash
  `a2ec83f9537b03c694f98ee6440bf66d28d722e1d74ba33a30fd7410115246aa`.
  Real field/button hit testing and keyboard/clipboard input save a bypass preset, load and Inspect it,
  render again, add an audio track at beat 4, then use Cmd-Z / Cmd-Shift-Z / Cmd-S. A fresh process verifies
  the persisted sample/clip/source. Rendered PCM before/after preset loading is identical: mono input
  duplicated to stereo, one second of 0.25 followed by 0.1 second of silence. Native boundary checks also
  cover a 129-frame partial block, seven tail frames, identity/state/parameter errors and create-only WAVs.
  The final `target/daw-vst3.png` was visually inspected. Audio was offline or simulated, with no device,
  listening, vendor-editor or third-party compatibility claim.

## Measurements and limitations

Apple M4, Darwin 27, Node 26.5, Rust 1.98.1 / LLVM 22.1.8; release helper, warm filesystem cache,
48 kHz / 128 frames / stereo, bypassed mono input. Three warmups and 20 samples; the preset contains
264 bytes of opaque state and two parameters. Timings include filesystem publication and fsync.

| Operation               | p95 ms | p99 ms |
| ----------------------- | -----: | -----: |
| Whole-helper Inspect    |  11.46 |  11.48 |
| One-second WAV render   |  13.22 |  13.26 |
| Create-only preset save |   8.70 |   8.83 |
| Preset load/validation  |  0.140 |  0.143 |

Archive: `benchmarks/results/2026-09-18-vst3-persistence.json`. The initial measurement, retained as
`2026-09-18-vst3-persistence-before-buffer-sizing.json`, used a full 8 MiB allocation for every read and
recorded load p95 0.610 ms and Inspect p95 12.80 ms. The final allocation is proportional to the file size;
the two runs do not isolate causality for helper timing. Final Inspect/render are within 10% of the preceding
helper baseline. Device, callback, CPU utilization and xrun remain unmeasured/null.

The existing source-daw benchmark also ran twice, with no parallel build/test load. Both complete all
transactions and fresh reopen. Archives are `2026-09-18-vst3-persistence-source-daw.json` and
`2026-09-18-vst3-persistence-source-daw-repeat.json`. Dirty Save p95 was **59.67 / 57.87 ms**, versus
**51.79 ms** in the September 9 baseline: **15.2% / 11.7% higher**. Whole-project edit p95 was
**165.41 / 215.39 ms**, versus **172.53 ms**. The latter repeat also exceeds the 10% budget. These are
unresolved performance observations, not a passed regression gate; this working tree also contains earlier
source-writing changes, and no differential experiment attributes the difference to this increment.
This existing benchmark does not measure the new frozen-audio import transaction or GPUI frame time.

Implementation responsibilities are split across preset I/O, state retention, audio asset preparation,
declarative authoring, GUI form actions/layout and GUI smoke. New source modules remain below 300 lines.
The pre-existing large ProjectDocument (889 lines) and Project (365 lines) receive small delegating branches;
new I/O/state/authoring logic lives in dedicated modules. Unrelated working-tree changes were preserved.
