import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";

export default tseslint.config(
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    plugins: {
      "react-hooks": reactHooks,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/no-unused-vars": [
        "error",
        { "varsIgnorePattern": "^_", "argsIgnorePattern": "^_", "caughtErrors": "none" },
      ],
      "react-hooks/set-state-in-effect": "warn",
    },
  },
  {
    // Test files run in Node.js — allow Node globals (URL, etc.)
    files: ["**/*.test.mjs"],
    languageOptions: {
      globals: {
        URL: "readonly",
      },
    },
  },
  {
    ignores: ["dist/", "src-tauri/"],
  },
);
