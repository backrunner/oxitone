import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// dist/gen/output.js → repo root (gen → dist → protocol → packages → root).
export const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "..");

export function write(rel: string, text: string): void {
  const path = join(repoRoot, rel);
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  console.log(`wrote ${rel}`);
}
