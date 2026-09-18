import { isBuiltin } from "node:module";
import { readFile, mkdir, rename, rm, writeFile } from "node:fs/promises";
import { dirname, extname, isAbsolute, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { build, context, transform, type BuildOptions, type BuildResult, type Loader, type Plugin } from "esbuild";

/** Inline local modules, preserving each module's source-relative asset URLs. */
export interface ProjectSourceLoader {
  /** Return a draft or the exact disk text, and record its read evidence before transformation. */
  read(path: string): Promise<string>;
  /** Resolve enrolled drafts with the same precedence as saved TypeScript modules. */
  resolveLocal?(specifier: string, directory: string, diskPath?: string): Promise<string | undefined>;
}
export function projectBuildOptions(entry: string, sourceLoader?: ProjectSourceLoader): BuildOptions {
  const sources: Plugin = {
    name: "oxitone-project-sources",
    setup(api) {
      if (sourceLoader?.resolveLocal)
        api.onResolve({ filter: /^(?:\.{1,2}\/|\/)/ }, async (args) => {
          if (args.pluginData?.resolved || args.kind === "entry-point") return;
          const found = await api.resolve(args.path, {
            importer: args.importer,
            resolveDir: args.resolveDir,
            kind: args.kind,
            pluginData: { resolved: true },
          });
          const path = await sourceLoader.resolveLocal!(
            args.path,
            args.resolveDir,
            found.errors.length ? undefined : found.path,
          );
          return path ? { path } : found;
        });
      api.onResolve({ filter: /^[^./]|^#/ }, async (args) => {
        if (args.pluginData?.resolved || args.kind === "entry-point" || isAbsolute(args.path)) return;
        if (isBuiltin(args.path) || args.path.startsWith("file:")) return { path: args.path, external: true };
        // Resolve in the author's package scope, using esbuild's ESM export conditions.
        const found = await api.resolve(args.path, {
          importer: args.importer,
          resolveDir: args.resolveDir,
          kind: args.kind,
          pluginData: { resolved: true },
        });
        if (found.errors.length) return { errors: found.errors };
        if (!found.path) return { errors: [{ text: `Cannot resolve ${args.path}` }] };
        // Package imports (#aliases) remain project code. Installed npm dependencies stay external.
        return args.path.startsWith("#")
          ? { path: found.path }
          : { path: pathToFileURL(found.path).href, external: true };
      });
      api.onLoad({ filter: /\.(?:[cm]?[jt]sx?|json)$/ }, async (args) => {
        if (extname(args.path) === ".json")
          return {
            contents: await (sourceLoader ? sourceLoader.read(args.path) : readFile(args.path, "utf8")),
            loader: "json",
          };
        const suffix = extname(args.path);
        const loader: Loader = suffix.endsWith("tsx")
          ? "tsx"
          : suffix.endsWith("jsx")
            ? "jsx"
            : suffix.includes("t")
              ? "ts"
              : "js";
        const result = await transform(
          await (sourceLoader ? sourceLoader.read(args.path) : readFile(args.path, "utf8")),
          {
            loader,
            sourcefile: args.path,
            format: "esm",
            target: "node24",
            sourcemap: "inline",
            define: {
              "import.meta.url": JSON.stringify(pathToFileURL(args.path).href),
              "import.meta.dirname": JSON.stringify(dirname(args.path)),
              "import.meta.filename": JSON.stringify(args.path),
            },
          },
        );
        return { contents: result.code, loader: "js", resolveDir: dirname(args.path), watchFiles: [args.path] };
      });
    },
  };
  return {
    absWorkingDir: dirname(resolve(entry)),
    entryPoints: [resolve(entry)],
    bundle: true,
    splitting: false,
    write: false,
    platform: "node",
    format: "esm",
    target: "node24",
    sourcemap: "inline",
    metafile: true,
    logLevel: "silent",
    outfile: "project.mjs",
    plugins: [sources],
  };
}

export function bundleCode(result: BuildResult, entry: string): string {
  if (result.outputFiles?.length !== 1) throw new Error("Project build must produce exactly one JavaScript file");
  const code = result.outputFiles[0]!.text;
  const hasOrigin = Object.values(result.metafile!.outputs).some((o) => o.exports.includes("__oxitoneSourceDirectory"));
  return hasOrigin
    ? code
    : `${code}\nexport const __oxitoneSourceDirectory = ${JSON.stringify(dirname(resolve(entry)))};\n`;
}

/** Publish only a successful complete bundle; failed rebuilds leave the previous file intact. */
export async function writeBundle(path: string, text: string): Promise<void> {
  await mkdir(dirname(path), { recursive: true });
  const temporary = `${path}.${process.pid}.tmp`;
  try {
    await writeFile(temporary, text);
    await rename(temporary, path);
  } finally {
    await rm(temporary, { force: true });
  }
}

export async function bundleProject(entry: string, output: string, watch = false): Promise<void> {
  entry = resolve(entry);
  output = resolve(output);
  if (entry === output || !output.endsWith(".mjs")) throw new Error("Build output must be a separate .mjs file");
  const options = projectBuildOptions(entry);
  options.outfile = output;
  const publish = async (result: Awaited<ReturnType<typeof build>>) => {
    // Do not let an explicit output overwrite any dependency of the project.
    if (Object.keys(result.metafile!.inputs).some((path) => resolve(options.absWorkingDir!, path) === output)) {
      throw new Error("Build output cannot overwrite an imported source file");
    }
    await writeBundle(output, bundleCode(result, entry));
    console.error(`Oxitone build · ${output}`);
  };
  if (!watch) {
    await publish(await build(options));
    return;
  }
  options.plugins!.push({
    name: "oxitone-build-output",
    setup(api) {
      api.onEnd(async (result) => {
        if (result.errors.length) console.error(`[PreviewBuildFailed] ${result.errors[0]!.text}`);
        else {
          try {
            await publish(result);
          } catch (error) {
            console.error(String(error));
          }
        }
      });
    },
  });
  const builder = await context(options);
  try {
    await builder.watch();
    await new Promise<void>((done) => {
      const stop = () => {
        process.off("SIGINT", stop);
        process.off("SIGTERM", stop);
        done();
      };
      process.once("SIGINT", stop);
      process.once("SIGTERM", stop);
    });
  } finally {
    await builder.dispose();
  }
}
