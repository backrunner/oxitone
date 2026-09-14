import { execFileSync } from "node:child_process";
import { mkdirSync, copyFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
process.chdir(fileURLToPath(new URL("../", import.meta.url)));
const rustc = execFileSync("rustup", ["which", "--toolchain", "stable", "rustc"], { encoding: "utf8" }).trim();
execFileSync(
  "rustup",
  ["run", "stable", "cargo", "build", "-p", "oxitone-wasm", "--release", "--target", "wasm32-unknown-unknown"],
  {
    stdio: "inherit",
    env: {
      ...process.env,
      RUSTC: rustc,
      CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS: "-C link-arg=--max-memory=1073741824",
    },
  },
);
mkdirSync("packages/web/dist", { recursive: true });
copyFileSync("target/wasm32-unknown-unknown/release/oxitone_wasm.wasm", "packages/web/dist/oxitone.wasm");
