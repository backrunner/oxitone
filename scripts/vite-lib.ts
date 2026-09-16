import { builtinModules } from "node:module";
import { existsSync, globSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { defineConfig, type UserConfig } from "vite";
import dts from "vite-plugin-dts";

export interface LibBuildOptions {
  /** Absolute package directory containing package.json and src/. */
  packageDir: string;
  /** esbuild-style build target; libraries default to es2022 like tsc. */
  target?: string;
  /** Empty dist before building; disable when runtime assets land in dist afterwards (wasm). */
  emptyOutDir?: boolean;
  /** Extra externals beyond dependencies/peerDependencies/node builtins. */
  externals?: (string | RegExp)[];
}

/**
 * Library build shared by every workspace package: vite/rolldown emits one ESM
 * module per source file (like `tsc -p tsconfig.build.json`) so relative
 * `import.meta.url` lookups (worker files, wasm, package resources) keep
 * working, while declarations come from vite-plugin-dts.
 */
export function libConfig(options: LibBuildOptions): UserConfig {
  const { packageDir } = options;
  const manifest = JSON.parse(readFileSync(join(packageDir, "package.json"), "utf8"));
  const deps = new Set([
    ...Object.keys(manifest.dependencies ?? {}),
    ...Object.keys(manifest.peerDependencies ?? {}),
    ...Object.keys(manifest.optionalDependencies ?? {}),
  ]);
  const extras = options.externals ?? [];
  const isExternal = (id: string) =>
    id.startsWith("node:") ||
    // Self-referencing subpath imports ("#foo") resolve through the package's
    // own `imports` map at runtime, with per-platform conditions — keep them.
    id.startsWith("#") ||
    builtinModules.includes(id) ||
    id.endsWith(".node") ||
    id.endsWith(".wasm") ||
    deps.has(id) ||
    [...deps].some((dep) => id.startsWith(`${dep}/`)) ||
    extras.some((x) => (typeof x === "string" ? x === id : x.test(id)));

  const srcRoot = join(packageDir, "src");
  const outDir = join(packageDir, "dist");
  const input = Object.fromEntries(
    globSync("**/*.ts", { cwd: srcRoot })
      .map((file) => file.replace(/\.ts$/, ""))
      .map((name) => [name, join(srcRoot, `${name}.ts`)]),
  );

  return defineConfig({
    root: packageDir,
    build: {
      // Build in the server-consumer environment so `new URL("...",
      // import.meta.url)` stays verbatim — the client pipeline inlines worker
      // files as data: URLs and mangles dynamic paths, which breaks spawned
      // workers and sibling-file resolution.
      ssr: true,
      outDir,
      emptyOutDir: options.emptyOutDir ?? true,
      sourcemap: true,
      minify: false,
      target: options.target ?? "es2022",
      // Every source file is a lib entry so preserveModules can emit the full
      // tree; rollupOptions.input alone lets rolldown drop re-export facades.
      lib: { entry: input, formats: ["es"] },
      rollupOptions: {
        external: isExternal,
        // These libraries run in Node AND browsers; neutral platform keeps the
        // emitted rolldown runtime free of node: imports so browser bundlers
        // (examples/web esbuild) can consume the output.
        platform: "neutral",
        // Library output must keep the declared export surface verbatim;
        // rolldown's shaker otherwise drops `export { x } from ...` facades.
        treeshake: false,
        output: {
          format: "es",
          preserveModules: true,
          preserveModulesRoot: "src",
          entryFileNames: "[name].js",
        },
      },
    },
    plugins: [
      dts({
        tsconfigPath: existsSync(join(packageDir, "tsconfig.build.json"))
          ? join(packageDir, "tsconfig.build.json")
          : join(packageDir, "tsconfig.json"),
        entryRoot: srcRoot,
        outDir,
      }),
    ],
  });
}
