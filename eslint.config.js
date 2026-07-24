import tseslint from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import importPlugin from "eslint-plugin-import";

export default [
  {
    ignores: [
      "**/dist/**",
      "**/node_modules/**",
      "**/coverage/**",
      "**/src-tauri/target/**",
      "**/*.config.js",
    ],
  },
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    plugins: {
      "@typescript-eslint": tseslint,
      import: importPlugin,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      "@typescript-eslint/no-unused-vars": "error",
      "import/no-restricted-paths": [
        "error",
        {
          zones: [
            {
              target: "./packages/core",
              from: "./packages/map",
              message: "core must not depend on map (Cesium-specific code).",
            },
            {
              target: "./packages/core",
              from: "./packages/ui",
              message: "core must not depend on ui.",
            },
            {
              target: "./packages/core",
              from: "./packages/storage-sqlite",
              message: "core must not depend on concrete storage adapters.",
            },
            {
              target: "./packages/core",
              from: "./packages/storage-indexeddb",
              message: "core must not depend on concrete storage adapters.",
            },
            {
              target: "./packages/core",
              from: "./packages/storage-remote",
              message: "core must not depend on concrete storage adapters.",
            },
          ],
        },
      ],
    },
  },
];
