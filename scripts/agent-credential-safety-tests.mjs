import assert from "node:assert/strict";
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
import {
  MAX_TEXT_BYTES,
  formatFindings,
  scanBlob,
  scanIndex,
  scanRepository,
  scanText,
  scanWorkingTree,
} from "./agent-credential-safety.mjs";

function specimen(...parts) {
  return parts.join("");
}

const sensitiveCases = [
  ["private-key", specimen("-----BEGIN ", "PRIVATE KEY-----")],
  ["api-token", specimen("sk", "-", "A".repeat(32))],
  ["bearer-token", specimen("Authorization: Bear", "er ", "b".repeat(32))],
  ["jwt-token", specimen("a".repeat(16), ".", "b".repeat(20), ".", "c".repeat(16))],
  ["oauth-literal", specimen('{"access_token":"', "live", "-value-", "x".repeat(20), '"}')],
  ["authorization-result", specimen("https://localhost/callback?co", "de=", "live-result-value")],
  ["email-address", specimen("person", "@", "company", ".com")],
];

for (const [expectedRule, value] of sensitiveCases) {
  const findings = scanText("candidate.txt", value);
  assert.ok(
    findings.some(({ rule }) => rule === expectedRule),
    `${expectedRule} must be detected`,
  );
}

const safeSourceAndDocs = `
const access_token = response.access_token;
const authorizationHeaderName = "Authorization";
Documentation may mention access tokens, refresh tokens, email addresses, and auth.json without values.
`;
assert.deepEqual(scanText("safe-source.rs", safeSourceAndDocs), []);

const syntheticFixture = specimen(
  '{"apiKey":"secret","access_token":"synthetic-access-value",',
  '"refresh_token":"fabricated-refresh-value",',
  '"account_id":"synthetic-account-value",',
  '"email":"tester@example.invalid",',
  '"authorization_code":"synthetic-authorization-result"}',
);
assert.deepEqual(scanText("fixture.json", syntheticFixture), []);

const redactedFinding = scanText(
  "candidate.txt",
  specimen("Bearer ", "never-print-this-value-", "z".repeat(20)),
);
const diagnostic = formatFindings(redactedFinding);
assert.match(diagnostic, /candidate\.txt: bearer-token/);
assert.doesNotMatch(diagnostic, /never-print-this-value/);

assert.deepEqual(scanBlob("asset.png", Buffer.from([0, 1, 2, 3])), []);
assert.deepEqual(scanBlob("unknown.bin", Buffer.from([0, 1, 2, 3])), []);
assert.deepEqual(
  scanBlob("large.txt", Buffer.alloc(MAX_TEXT_BYTES + 1, 0x61)),
  [{ path: "large.txt", rule: "unscannable-large-text" }],
);
assert.deepEqual(scanBlob("small.txt", Buffer.from("ordinary text")), []);

for (const path of ["auth.json", ".env", "nested/credentials.json", "private.pem"]) {
  const findings = scanBlob(path, Buffer.from("ordinary text"));
  assert.ok(findings.some(({ rule }) => rule === "prohibited-credential-path"));
}

const repository = mkdtempSync(join(tmpdir(), "agent-credential-safety-"));
function git(...args) {
  const result = spawnSync("git", args, { cwd: repository, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
}
git("init", "--quiet");
writeFileSync(join(repository, "tracked.txt"), "safe index content\n");
git("add", "tracked.txt");
git("-c", "user.name=Synthetic Test", "-c", "user.email=tester@example.invalid", "commit", "--quiet", "-m", "initial");

writeFileSync(
  join(repository, "tracked.txt"),
  specimen("Bearer ", "working-tree-only-", "q".repeat(24)),
);
assert.deepEqual(scanIndex(repository), []);
const workingTreeFindings = scanWorkingTree(repository);
assert.ok(workingTreeFindings.some(({ rule }) => rule === "bearer-token"));
assert.doesNotMatch(formatFindings(workingTreeFindings), /working-tree-only/);
assert.ok(scanRepository(repository).some(({ rule }) => rule === "bearer-token"));

git("add", "tracked.txt");
writeFileSync(join(repository, "tracked.txt"), "safe working tree content\n");
const stagedFindings = scanRepository(repository);
assert.ok(stagedFindings.some(({ rule }) => rule === "bearer-token"));
assert.doesNotMatch(formatFindings(stagedFindings), /working-tree-only/);

writeFileSync(
  join(repository, "new-untracked.txt"),
  specimen("sk", "-", "n".repeat(32)),
);
assert.ok(
  scanWorkingTree(repository).some(
    ({ path, rule }) => path === "new-untracked.txt" && rule === "api-token",
  ),
);

git("add", "new-untracked.txt");
assert.ok(
  scanIndex(repository).some(
    ({ path, rule }) => path === "new-untracked.txt" && rule === "api-token",
  ),
);

if (process.platform !== "win32") {
  const symlink = join(repository, "untracked-link.txt");
  const target = join(repository, "outside.txt");
  writeFileSync(target, "ordinary external text\n");
  symlinkSync(target, symlink);
  assert.ok(
    scanWorkingTree(repository).some(
      ({ path, rule }) => path === "untracked-link.txt" && rule === "unscannable-symlink",
    ),
  );
}
rmSync(repository, { recursive: true, force: true });

console.log("agent credential safety self-tests passed");
