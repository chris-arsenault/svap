import js from "@eslint/js";
import globals from "globals";
import react from "eslint-plugin-react";
import reactHooks from "eslint-plugin-react-hooks";
import reactRefresh from "eslint-plugin-react-refresh";
import reactPerf from "eslint-plugin-react-perf";
import jsxA11y from "eslint-plugin-jsx-a11y";
import sonarjs from "eslint-plugin-sonarjs";
import prettier from "eslint-config-prettier";
import tseslint from "typescript-eslint";
import maxJsxProps from "./eslint-rules/max-jsx-props.js";
import noInlineStyles from "./eslint-rules/no-inline-styles.js";
import noDirectStoreImport from "./eslint-rules/no-direct-store-import.js";
import noDirectFetch from "./eslint-rules/no-direct-fetch.js";
import noEscapeHatches from "./eslint-rules/no-escape-hatches.js";
import noManualAsyncState from "./eslint-rules/no-manual-async-state.js";
import noManualViewHeader from "./eslint-rules/no-manual-view-header.js";
import noManualExpandState from "./eslint-rules/no-manual-expand-state.js";

export default tseslint.config(
  {
    ignores: ["node_modules/", "dist/"],
  },

  // Base JS rules + complexity limits
  {
    ...js.configs.recommended,
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "module",
    },
    rules: {
      complexity: ["warn", 10],
      "max-lines": ["warn", { max: 400, skipBlankLines: true, skipComments: true }],
      "max-lines-per-function": ["warn", { max: 75, skipBlankLines: true, skipComments: true }],
      "max-depth": ["warn", 4],
    },
  },

  // TypeScript: recommended rules
  ...tseslint.configs.recommended,

  // React + Browser + Accessibility
  {
    files: ["src/**/*.{ts,tsx}"],
    plugins: {
      react,
      "react-hooks": reactHooks,
      "react-refresh": reactRefresh,
      "react-perf": reactPerf,
      "jsx-a11y": jsxA11y,
      local: { rules: { "max-jsx-props": maxJsxProps, "no-inline-styles": noInlineStyles, "no-direct-store-import": noDirectStoreImport, "no-direct-fetch": noDirectFetch, "no-escape-hatches": noEscapeHatches, "no-manual-async-state": noManualAsyncState, "no-manual-view-header": noManualViewHeader, "no-manual-expand-state": noManualExpandState } },
    },
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.es2025,
      },
      parserOptions: {
        ecmaFeatures: { jsx: true },
      },
    },
    settings: {
      react: { version: "detect" },
    },
    rules: {
      ...react.configs.recommended.rules,
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.configs.recommended.rules,
      "react/react-in-jsx-scope": "off",
      "react/prop-types": "off",
      "react/no-unescaped-entities": "warn",
      "react-refresh/only-export-components": ["warn", { allowConstantExport: true }],
      "@typescript-eslint/no-unused-vars": ["warn", { argsIgnorePattern: "^_" }],
      "no-unused-vars": "off",
      "react-perf/jsx-no-new-object-as-prop": ["warn", { nativeAllowList: "all" }],
      "react-perf/jsx-no-new-array-as-prop": ["warn", { nativeAllowList: "all" }],
      "react-perf/jsx-no-new-function-as-prop": ["warn", { nativeAllowList: "all" }],
      "react-perf/jsx-no-jsx-as-prop": ["warn", { nativeAllowList: "all" }],
      "react/jsx-no-constructed-context-values": "warn",
      "local/max-jsx-props": ["warn", { max: 12 }],
      "local/no-inline-styles": "warn",
      "local/no-direct-store-import": "warn",
      "local/no-direct-fetch": "warn",
      "local/no-escape-hatches": "warn",
      "local/no-manual-async-state": "warn",
      "local/no-manual-view-header": "warn",
      "local/no-manual-expand-state": "warn",
    },
  },

  // SonarJS
  sonarjs.configs.recommended,

  // Prettier must be last
  prettier,
);
