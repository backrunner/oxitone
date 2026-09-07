import { fileURLToPath } from "node:url";
// Works from both src/full and dist/full. Generated audio and libraries stay ignored.
export const outputRoot = fileURLToPath(new URL("../../../../target/examples/full-songs/", import.meta.url));
