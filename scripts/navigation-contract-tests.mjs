import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import * as esbuild from "esbuild";

const tempDir = await mkdtemp(join(tmpdir(), "job-radar-navigation-tests-"));
const outfile = join(tempDir, "navigation-contract-tests.mjs");

try {
  await esbuild.build({
    entryPoints: ["src/app/navigation/navigation-contract-tests.tsx"],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: "node25",
    logLevel: "silent",
    alias: { "@": "./src" },
  });
  await import(pathToFileURL(outfile).href);
  console.log("navigation contract tests passed");
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
