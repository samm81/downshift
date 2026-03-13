const js = require("@eslint/js");
const globals = require("globals");

module.exports = [
  {
    ignores: ["docs/**"],
  },
  js.configs.recommended,
  {
    files: ["src/ui/**/*.js"],
    languageOptions: {
      ecmaVersion: "latest",
      sourceType: "script",
      globals: {
        ...globals.browser,
      },
    },
    rules: {
      "no-unused-vars": [
        "error",
        {
          args: "none",
          caughtErrors: "none",
        },
      ],
    },
  },
];
