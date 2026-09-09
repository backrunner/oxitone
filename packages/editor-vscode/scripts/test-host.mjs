import { build } from "esbuild";
import { runTests } from "@vscode/test-electron";
import { mkdir, mkdtemp, symlink, writeFile, rm } from "node:fs/promises";
import { resolve, join } from "node:path";

const root = await mkdtemp("/tmp/oxitone-vscode-");
try {
  await mkdir(join(root, "node_modules/@oxitone"), { recursive: true });
  await symlink(resolve("../core"), join(root, "node_modules/@oxitone/core"), "dir");
  await writeFile(join(root, "song.ts"), "import { Project, chord } from '@oxitone/core'; const phrase = chord(60, 'major'); const project = new Project(); project.addTrack('Lead').add(phrase).at({ bar: 1 }); export default project;\n");
  await build({ entryPoints: ["test/host/index.ts"], outfile: "dist/host.cjs", bundle: true, platform: "node", format: "cjs", target: "node20", external: ["vscode", "@oxitone/cli/editor"] });
  await runTests({ version: "1.96.4", cachePath: resolve("../../target/vscode-test"),
    extensionDevelopmentPath: process.cwd(), extensionTestsPath: resolve("dist/host.cjs"),
    extensionTestsEnv: { OXITONE_TEST_ROOT: root, OXITONE_TEST_NODE: process.execPath, OXITONE_TEST_SERVICE: resolve("scripts/test-service.mjs"),
      OXITONE_TEST_CLI: resolve("../cli/dist/index.js"), OXITONE_TEST_VIEWER: resolve("../../target/debug/oxitone-preview") },
    launchArgs: [root, "--disable-extensions", "--skip-welcome", "--skip-release-notes", "--disable-workspace-trust", "--disable-updates", "--disable-telemetry", "--user-data-dir", join(root, "user-data"), "--extensions-dir", join(root, "extensions")],
  });
} finally { await rm(root, { recursive: true, force: true }); }
