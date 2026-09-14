# @oxitone/samples

Import a local audio file into an Oxitone project. Rust owns file inspection,
SHA-256 and decoding; JavaScript receives metadata and never receives PCM.

```ts
import { Project } from "@oxitone/core";
import { importSample } from "@oxitone/samples";

const project = new Project();
const sample = project.addSample(importSample("/music/loop.wav"));
project.addTrack("audio").use(project.addChannel()).sample(sample).at({ bar: 1 });
await project.renderWav({ path: "/music/mix.wav", tailSeconds: 0 });
```

`importSample(path, { assetBaseDir? })` is synchronous. It supports the Rust
WAV, AIFF, FLAC, MP3 and MP4/M4A audio decoders. It returns `assetUri`, `sha256`,
`format`, `sampleRate`, `channels`, bigint `frames` and `provenance` containing
original channel count, optional bit depth, decoder and downmix action.

Relative input uses the current directory by default and produces an absolute
URI. With `assetBaseDir`, input is resolved against that directory and produces
a relative URI; files outside the base are rejected. Pass the corresponding
`assetBaseDir` to `renderWav` when using relative descriptors. `Project.compile`
and playback currently use absolute URIs. Paths are local paths, not file URLs.

Use `{ ...importSample(path), musicalLengthBeats: 8, edits: { ... } }` to add
authoring metadata. For inspection alone, `inspectSample(path)` returns the
versioned wire metadata (`frames` is a decimal string) without requiring an engine.

Import reads and fully decodes the source, then discards PCM. It does not write
files or retain a cache. Prepare verifies the original hash and decodes again;
changing a source requires re-import. Sources with more than two channels are
downmixed to stereo. Compressed dimensions can include codec padding.
Provenance is returned to the caller but is not yet stored in project snapshots.

Errors use stable Oxitone codes: `InvalidProject` for invalid paths,
`AssetUnavailable` for unreadable files, and `SampleFormatUnsupported` for
unrecognized, undecodable or empty audio. File errors include `details.path`.
