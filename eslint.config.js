import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import globals from "globals";
import tseslint from "typescript-eslint";

export default tseslint.config(
  {
    ignores: [
      "dist",
      "node_modules",
      "src-tauri/target",
      "src-tauri/gen",
      "vite.config.ts",
    ],
  },
  js.configs.recommended,
  {
    files: ["scripts/**/*.mjs", "*.config.js"],
    languageOptions: {
      globals: {
        ...globals.node,
      },
    },
  },
  {
    files: ["src/**/*.{ts,tsx}", "*.config.ts"],
    extends: [
      ...tseslint.configs.strictTypeChecked,
      ...tseslint.configs.stylisticTypeChecked,
    ],
    languageOptions: {
      parserOptions: {
        project: ["./tsconfig.json", "./tsconfig.node.json"],
        tsconfigRootDir: import.meta.dirname,
      },
      globals: {
        ...globals.browser,
        ...globals.es2024,
      },
    },
    plugins: {
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/consistent-type-imports": [
        "error",
        { prefer: "type-imports" },
      ],
      "@typescript-eslint/no-explicit-any": "error",
      "@typescript-eslint/no-unsafe-type-assertion": "error",
      "@typescript-eslint/restrict-template-expressions": [
        "error",
        { allowNumber: true, allowBoolean: true },
      ],
      complexity: ["warn", 10],
      "max-lines-per-function": [
        "warn",
        { max: 60, skipBlankLines: true, skipComments: true },
      ],
      "no-restricted-imports": [
        "error",
        {
          patterns: [
            {
              group: ["@tauri-apps/api", "@tauri-apps/api/*"],
              message:
                "Use src/ipc as the only frontend wrapper around Tauri APIs.",
            },
          ],
        },
      ],
      "react-refresh/only-export-components": [
        "warn",
        { allowConstantExport: true },
      ],
    },
  },
  {
    files: ["src/ipc/**/*.{ts,tsx}"],
    rules: {
      "no-restricted-imports": "off",
    },
  },
  {
    // src/components/ui holds shadcn/ui primitives, a component library
    // rather than Fast Refresh boundaries. These files intentionally
    // re-export cva variant helpers and Radix members alongside
    // components, which the rule cannot distinguish from app code.
    files: ["src/components/ui/**/*.{ts,tsx}"],
    rules: {
      "react-refresh/only-export-components": "off",
    },
  },
);
