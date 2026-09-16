import { libConfig } from "../../scripts/vite-lib.ts";

// build-wasm.mjs copies oxitone.wasm into dist after the vite build (prepack
// order and the CI workflow both run it last).
export default libConfig({ packageDir: import.meta.dirname });
