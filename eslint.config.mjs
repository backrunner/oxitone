import js from "@eslint/js";
import globals from "globals";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/target/**",
      "**/coverage/**",
      "schemas/**",
      "packages/native-generated/**",
      ".agents/**",
      "tmp/**",
      // Rust land — cargo fmt/clippy own it; NAPI-RS emits index.js/.d.ts.
      "crates/**",
      "apps/**",
      "include/**",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    // Plain JS files are Node tooling scripts (build, smoke, benchmarks).
    files: ["**/*.{js,mjs,cjs}"],
    languageOptions: { globals: globals.node },
  },
  {
    // Web example smokes drive Playwright from Node and evaluate page code.
    files: ["examples/web/**/*.mjs"],
    languageOptions: { globals: { ...globals.node, ...globals.browser } },
  },
  {
    rules: {
      eqeqeq: ["error", "always", { null: "ignore" }],
      "no-var": "error",
      "object-shorthand": "error",
      "prefer-const": "error",
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
    },
  },
  // Last: turns off every stylistic rule that overlaps with prettier.
  prettier,
);
