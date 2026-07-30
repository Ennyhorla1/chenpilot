"use strict";
import js from "@eslint/js";
import globals from "globals";
import tseslint from "typescript-eslint";
import jsdoc from "eslint-plugin-jsdoc";
export default [
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["**/*.{js,mjs,cjs,ts,mts,cts}"],
    languageOptions: {
      globals: Object.assign(Object.assign({}, globals.node), globals.es2021),
    },
    rules: {
      "@typescript-eslint/no-namespace": "off",
      "@typescript-eslint/no-empty-object-type": "off",
      "@typescript-eslint/no-explicit-any": "warn",
    },
  },
  {
    files: ["src/Agents/tools/*.ts"],
    plugins: {
      jsdoc
    },
    rules: {
      "jsdoc/require-jsdoc": ["error", {
        require: { ClassDeclaration: true, MethodDefinition: true, FunctionDeclaration: true, ArrowFunctionExpression: false, FunctionExpression: false }
      }]
    }
  },
  {
    ignores: ["node_modules/**", "dist/**", "build/**"],
  },
    {
        ignores: ["node_modules/**", "dist/**", "build/**", "packages/bot/src/**/*.js"]
    }
];
