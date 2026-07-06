import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import * as esbuild from "esbuild";

const tempDir = await mkdtemp(join(tmpdir(), "job-radar-postings-ui-tests-"));
const outfile = join(tempDir, "postings-ui-contract-tests.mjs");

try {
  await esbuild.build({
    entryPoints: ["src/features/postings/tests/postings-ui-contract-tests.ts"],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: "node25",
    logLevel: "silent",
    alias: { "@": "./src" },
  });
  await import(pathToFileURL(outfile).href);
  console.log("postings UI contract tests passed");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
