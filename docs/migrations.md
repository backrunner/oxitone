# Compatibility and migration notes

The current project format and native protocol are `1.0`. These authoring additions
do not require a wire major bump. Native plugin versions are separate from npm
package versions. Unknown major format/protocol versions are rejected; no major
migration is currently needed or implemented.

| Earlier usage                              | Current API / behavior                                                                                                                 |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| Installing several authoring packages      | `oxitone` re-exports authoring, presets, sample import and the existing native API; lower layers use `@oxitone/native` to avoid cycles |
| Per-engine-only plugin registration        | `Project.registerPlugin` retains verified libraries for future Project engines; existing Session policy still applies                  |
| Copying parameter objects between projects | Versioned presets validate native descriptors and can atomically apply Channel settings/resources                                      |
| Manual preview scripts                     | `oxitone preview <entry.ts>` supplies GPUI views and dependency watch; factories run in ESM worker processes                           |
| Hand-built builtin InstrumentRefs          | `wavetable`, `sampler`, `slicer` return the same declarative references with typed controls                                            |
| JSON snapshots only                        | `Project.save` and `Project.load` preserve portable assets and editable builders                                                       |
| Restoring an authoring model manually      | `Project.fromSnapshot(snapshot, {assetBaseDir?})` retains IDs, exact beats, order and revision                                         |
| Number-only revision access                | Prefer `revisionBigInt` for persistence; the number getter rejects unsafe integers                                                     |
| Metadata-only sample import                | Existing `importSample(path)` stays read-only; add `cacheDir` for normalized WAV publication                                           |
| Source hash used for a cached asset        | Use returned top-level hash/format for playback; original metadata is under provenance                                                 |
| Editing a compiled Project                 | Call `session.update()`; `Project.play()` also refreshes the current authoring revision                                                |
| Manual Slicer `tempoFactor`                | Use `state.tempoSync: 'repitch'` or the helper option; tempoFactor is host-owned and direct assignments now fail                       |
| Unknown insert automation paths            | Use `insert.<index>.parameter.<id>`, `insert.<index>.mix` or `.bypass`                                                                 |
| CLI snapshots with cwd-relative assets     | Relative assets now resolve beside the input JSON; project directories and their standard manifests are accepted too                   |
| Parsing CLI error text                     | Errors now emit JSON on stderr, including stable code/details when available                                                           |

Old snapshots may omit provenance and Slicer tempoSync; omission retains read-only
source metadata behavior and off playback respectively. Slicer repitch now follows
the effective Project tempo. Saved projects that explicitly requested repitch can
therefore sound different from the earlier implementation, which ignored that mode.
Sampler/Slicer seek now restarts the ADSR attack consistently; exact audio hashes
after seek can change to match a fresh render.

No caller-owned source files are rewritten by import or project restoration. Save
is explicit. Unknown extension fields are currently dropped by schema parsing;
round-tripping arbitrary third-party project metadata is not supported yet.

The unified authoring facade is implemented in `packages/sdk`; the old low-level
facade moved to package name `@oxitone/native` and remains re-exported from `oxitone`.
macOS npm platform distribution is still pending. Do not treat workspace success
as npm release acceptance. Engine policy and parameter smoothing strings now
serialize with the public hyphenated spellings; Rust accepts the old camelCase aliases.
