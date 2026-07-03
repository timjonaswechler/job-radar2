import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import * as esbuild from "esbuild";

const tempDir = await mkdtemp(join(tmpdir(), "job-radar-source-ui-tests-"));
const outfile = join(tempDir, "source-ui-contract-tests.mjs");

try {
  await esbuild.build({
    entryPoints: ["src/features/sources/source-ui-contract-tests.ts"],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: "node25",
    logLevel: "silent",
  });
  await import(pathToFileURL(outfile).href);
  console.log("source UI contract tests passed");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
