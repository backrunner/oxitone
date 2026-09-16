import { builtinModules } from "node:module";
import { defineConfig } from "vite";

export default defineConfig({
  build: {
    outDir: "dist",
    emptyOutDir: true,
    sourcemap: true,
    minify: false,
    target: "node20",
    lib: {
      entry: "src/extension.ts",
      formats: ["cjs"],
      fileName: () => "extension.cjs",
    },
    rollupOptions: {
      external: (id) => id === "vscode" || id.startsWith("node:") || builtinModules.includes(id),
    },
  },
});
