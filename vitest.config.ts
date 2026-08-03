import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    environment: "happy-dom",
    fileParallelism: false,
    include: ["src/**/*.component.test.tsx"],
    maxWorkers: 1,
    pool: "threads",
    setupFiles: ["./src/test/tauriMocks.ts"],
  },
});
