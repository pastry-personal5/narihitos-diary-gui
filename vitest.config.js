import { defineConfig } from "vitest/config";
export default defineConfig({
  test: {
    include: ["tests/**/*.test.{js,ts,tsx}"],
    // The published Atomic package has bundler-style extensionless ESM imports.
    // Exercise the actual package through Vite, as in the production bundle.
    server: { deps: { inline: ["@atomic-editor/editor"] } },
  },
});
