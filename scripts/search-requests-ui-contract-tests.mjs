import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import * as esbuild from "esbuild";

const tempDir = await mkdtemp(join(tmpdir(), "job-radar-search-requests-ui-tests-"));
const outfile = join(tempDir, "search-requests-ui-contract-tests.mjs");

try {
  await esbuild.build({
    entryPoints: ["src/features/search-requests/tests/search-requests-ui-contract-tests.ts"],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: "node25",
    alias: {
      "@": "./src",
    },
    logLevel: "silent",
  });
  await import(pathToFileURL(outfile).href);
  console.log("search requests UI contract tests passed");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
