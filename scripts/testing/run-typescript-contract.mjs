import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { pathToFileURL } from "node:url";

import * as esbuild from "esbuild";

const [entryPoint, label] = process.argv.slice(2);

if (!entryPoint || !label) {
  console.error(
    "Usage: node scripts/testing/run-typescript-contract.mjs <entry-point> <label>",
  );
  process.exit(2);
}

const tempDir = await mkdtemp(join(tmpdir(), "job-radar-contract-test-"));
const outfile = join(tempDir, `${basename(entryPoint).replace(/\.[^.]+$/, "")}.mjs`);

try {
  await esbuild.build({
    entryPoints: [entryPoint],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    target: `node${process.versions.node.split(".")[0]}`,
    alias: { "@": "./src" },
    logLevel: "silent",
  });
  await import(pathToFileURL(outfile).href);
  console.log(`${label} passed`);
} finally {
  await rm(tempDir, { recursive: true, force: true });
}
