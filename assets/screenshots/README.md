# Native preview screenshots

`preview-dark.png` and `preview-light.png` show the real GPUI window running
[Midnight Circuit](../../examples/drum-machine), with the arrangement and piano roll.
The README selects the matching image for the reader's color scheme.

Recreate them from the repository root after `pnpm install --frozen-lockfile`,
`pnpm build` and `pnpm example:drums`:

```sh
node scripts/build-preview.mjs --debug
mkdir -p target/readme-screenshots
OXITONE_PREVIEW_CAPTURE="$PWD/target/readme-screenshots/preview-dark.png" \
  OXITONE_PREVIEW_CAPTURE_SIZE=1440x920 OXITONE_PREVIEW_APPEARANCE=dark \
  node packages/cli/dist/index.js preview examples/drum-machine/src/preview.ts \
  --no-watch --viewer "target/debug/Oxitone Preview.app"
OXITONE_PREVIEW_CAPTURE="$PWD/target/readme-screenshots/preview-light.png" \
  OXITONE_PREVIEW_CAPTURE_SIZE=1440x920 OXITONE_PREVIEW_APPEARANCE=light \
  node packages/cli/dist/index.js preview examples/drum-machine/src/preview.ts \
  --no-watch --viewer "target/debug/Oxitone Preview.app"
```

Capture mode forces a simulated audio sink and changes only the window's appearance.
It saves the native window and closes it automatically. Inspect both PNGs before
copying them here; these are application screenshots, with no composited UI.
