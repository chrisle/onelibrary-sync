import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import tseslint from "typescript-eslint";

export default tseslint.config(
  { ignores: ["dist", "src-tauri/target", "target"] },
  js.configs.recommended,
  ...tseslint.configs.recommendedTypeChecked,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: {
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
      globals: { window: "readonly", document: "readonly", console: "readonly" },
    },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_", varsIgnorePattern: "^_" }],
      "no-console": ["error", { allow: ["warn", "error"] }],
      "no-restricted-syntax": ["error", {
        selector: "CallExpression[callee.name='setInterval']",
        message: "Schedule the next check with setTimeout once the last one has finished.",
      }],
    },
  },
  {
    // `invoke` only inside src/ipc, so the mock can stand in wholesale.
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/ipc/**"],
    rules: {
      "no-restricted-imports": ["error", { paths: [{ name: "@tauri-apps/api/core", message: "invoke belongs in src/ipc/." }] }],
    },
  },
  {
    files: ["**/*.test.ts", "**/*.test.tsx"],
    rules: {
      "@typescript-eslint/no-unsafe-assignment": "off",
      "@typescript-eslint/no-unsafe-member-access": "off",
      "@typescript-eslint/no-unsafe-call": "off",
      // act(async () => …) flushes effects even when nothing is awaited.
      "@typescript-eslint/require-await": "off",
    },
  },
);
