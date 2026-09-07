import { access, chmod, lstat, mkdir, readlink, symlink } from "node:fs/promises";
import { constants } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const entry = fileURLToPath(new URL("../packages/cli/dist/index.js", import.meta.url));
await access(entry, constants.R_OK);
const directory = resolve(process.argv[2] ?? join(homedir(), ".local", "bin"));
const executable = join(directory, "oxitone");
await mkdir(directory, { recursive: true });
await chmod(entry, 0o755);
try {
  const info = await lstat(executable);
  if (!info.isSymbolicLink() || resolve(dirname(executable), await readlink(executable)) !== entry) {
    throw new Error(`Existing ${executable} belongs to another installation; choose another bin directory`);
  }
} catch (error) {
  if (error.code !== "ENOENT") throw error;
  await symlink(entry, executable);
}
console.log(`Installed ${executable} -> ${entry}`);
if (!(process.env.PATH ?? "").split(delimiter).includes(directory)) {
  console.log(`Add this directory to your shell PATH: ${directory}`);
} else console.log("Ready: oxitone --help");
