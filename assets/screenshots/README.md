# Native preview screenshots

`preview-dark.png` and `preview-light.png` show the real GPUI window running
[Midnight Circuit](../../examples/drum-machine), with titlebar transport, arrangement,
chord piano roll and mixer. Captured from the current native app on 2026-10-08.
The README selects the matching image for the reader's color scheme.

Recreate them from the repository root after `pnpm install --frozen-lockfile`,
`pnpm build` and `pnpm example:drums`:

```sh
node scripts/build-preview.mjs --debug
mkdir -p target/readme-screenshots
OXITONE_PREVIEW_CAPTURE="$PWD/target/readme-screenshots/preview-dark.png" \
  OXITONE_PREVIEW_CAPTURE_SIZE=1440x920 OXITONE_PREVIEW_APPEARANCE=dark \
  OXITONE_PREVIEW_CAPTURE_LAYOUT=readme \
  node packages/cli/dist/index.js preview examples/drum-machine/src/preview.ts \
  --no-watch --viewer "target/debug/Oxitone Preview.app"
OXITONE_PREVIEW_CAPTURE="$PWD/target/readme-screenshots/preview-light.png" \
  OXITONE_PREVIEW_CAPTURE_SIZE=1440x920 OXITONE_PREVIEW_APPEARANCE=light \
  OXITONE_PREVIEW_CAPTURE_LAYOUT=readme \
  node packages/cli/dist/index.js preview examples/drum-machine/src/preview.ts \
  --no-watch --viewer "target/debug/Oxitone Preview.app"
```

Capture mode forces a simulated audio sink. The `readme` layout selects the second
pattern and opens the piano/mixer dock using view preferences; source and audio data
remain unchanged, and transport stays stopped.
It saves the native window and closes it automatically. Inspect both PNGs before
copying them here; these are application screenshots, with no composited UI.
