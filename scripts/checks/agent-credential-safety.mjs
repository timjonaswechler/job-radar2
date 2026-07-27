import { spawnSync } from "node:child_process";
import { lstatSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

export const MAX_TEXT_BYTES = 2 * 1024 * 1024;

const binaryExtensions = new Set([
  ".bmp",
  ".db",
  ".gif",
  ".icns",
  ".ico",
  ".jpeg",
  ".jpg",
  ".pdf",
  ".png",
  ".sqlite",
  ".webp",
  ".zip",
]);

const literalCredentialKeys = [
  "access",
  "access_token",
  "accountId",
  "account_id",
  "api_key",
  "apiKey",
  "authorization_code",
  "authorization_result",
  "code_verifier",
  "device_auth_id",
  "email",
  "refresh",
  "refresh_token",
];

const rules = [
  {
    rule: "private-key",
    expression: new RegExp(["-----BEGIN ", "(?:RSA |EC |OPENSSH )?", "PRIVATE KEY-----"].join(""), "g"),
  },
  {
    rule: "api-token",
    expression: new RegExp(["s", "k-", "[A-Za-z0-9_-]{20,}"].join(""), "g"),
  },
  {
    rule: "bearer-token",
    expression: new RegExp(["(?:Authorization\\s*:\\s*)?", "Bear", "er\\s+", "([A-Za-z0-9._~+/-]{20,}=*)"].join(""), "gi"),
    valueGroup: 1,
  },
  {
    rule: "jwt-token",
    expression: new RegExp("(?:^|[^A-Za-z0-9_-])([A-Za-z0-9_-]{12,}\\.[A-Za-z0-9_-]{12,}\\.[A-Za-z0-9_-]{12,})(?=$|[^A-Za-z0-9_-])", "g"),
    valueGroup: 1,
  },
  {
    rule: "oauth-literal",
    expression: new RegExp(
      `["'](?:${literalCredentialKeys.join("|")})["']\\s*:\\s*["']([^"']+)["']`,
      "gi",
    ),
    valueGroup: 1,
  },
  {
    rule: "authorization-result",
    expression: new RegExp(["(?:[?&]|^)co", "de=", "([^&\\s]{8,})"].join(""), "gi"),
    valueGroup: 1,
  },
  {
    rule: "email-address",
    expression: new RegExp("[A-Za-z][A-Za-z0-9._%+-]*@([A-Za-z][A-Za-z0-9.-]*\\.[A-Za-z]{2,})", "g"),
    valueGroup: 0,
  },
];

function extension(path) {
  const basename = path.toLowerCase().split("/").at(-1) ?? "";
  const index = basename.lastIndexOf(".");
  return index >= 0 ? basename.slice(index) : "";
}

function prohibitedPath(path) {
  const basename = path.toLowerCase().split("/").at(-1) ?? "";
  return (
    basename === "auth.json" ||
    basename === "credential.json" ||
    basename === "credentials.json" ||
    basename === "secret.json" ||
    basename === "secrets.json" ||
    basename === ".env" ||
    basename.startsWith(".env.") ||
    [".key", ".p12", ".pem", ".pfx"].includes(extension(path))
  );
}

function conspicuouslySynthetic(value) {
  const lower = value.toLowerCase();
  if (
    lower.includes("synthetic") ||
    lower.includes("fabricated") ||
    ["dummy", "example", "placeholder", "secret", "test"].includes(lower)
  ) {
    return true;
  }
  if (lower.includes("@")) {
    const domain = lower.split("@").at(-1) ?? "";
    return domain.endsWith(".invalid") || domain.endsWith(".test");
  }
  return false;
}

export function scanText(path, text) {
  const findings = [];
  for (const { rule, expression, valueGroup } of rules) {
    expression.lastIndex = 0;
    for (const match of text.matchAll(expression)) {
      const value = match[valueGroup ?? 0] ?? "";
      if (!conspicuouslySynthetic(value)) {
        findings.push({ path, rule });
        break;
      }
    }
  }
  return findings;
}

export function scanBlob(path, content) {
  const findings = [];
  if (prohibitedPath(path)) {
    findings.push({ path, rule: "prohibited-credential-path" });
  }
  if (binaryExtensions.has(extension(path)) || content.includes(0)) {
    return findings;
  }
  if (content.length > MAX_TEXT_BYTES) {
    findings.push({ path, rule: "unscannable-large-text" });
    return findings;
  }
  return findings.concat(scanText(path, content.toString("utf8")));
}

function runGit(repository, args, options = {}) {
  const result = spawnSync("git", args, {
    cwd: repository,
    encoding: options.encoding,
    maxBuffer: options.maxBuffer ?? 32 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error("credential safeguard could not read the Git index");
  }
  return result.stdout;
}

export function scanIndex(repository = process.cwd()) {
  const output = runGit(repository, ["ls-files", "--stage", "-z"]);
  const findings = [];
  const seen = new Set();
  for (const rawEntry of output.toString("utf8").split("\0")) {
    if (!rawEntry) continue;
    const tab = rawEntry.indexOf("\t");
    if (tab < 0) throw new Error("credential safeguard received an invalid Git index entry");
    const metadata = rawEntry.slice(0, tab).split(" ");
    const path = rawEntry.slice(tab + 1);
    const [mode, objectId, stage] = metadata;
    if (stage !== "0") {
      findings.push({ path, rule: "unmerged-index-entry" });
      continue;
    }
    if (seen.has(path)) continue;
    seen.add(path);
    if (mode === "160000") continue;
    if (binaryExtensions.has(extension(path))) {
      if (prohibitedPath(path)) findings.push({ path, rule: "prohibited-credential-path" });
      continue;
    }
    const size = Number(runGit(repository, ["cat-file", "-s", objectId], { encoding: "utf8" }));
    if (!Number.isSafeInteger(size) || size < 0) {
      throw new Error("credential safeguard received an invalid Git object size");
    }
    if (size > MAX_TEXT_BYTES) {
      findings.push({ path, rule: "unscannable-large-text" });
      continue;
    }
    const content = runGit(repository, ["cat-file", "blob", objectId], {
      maxBuffer: Math.max(MAX_TEXT_BYTES + 1, size + 1),
    });
    findings.push(...scanBlob(path, content));
  }
  return findings;
}

export function scanWorkingTree(repository = process.cwd()) {
  const output = runGit(repository, ["ls-files", "--cached", "--others", "--exclude-standard", "-z"]);
  const findings = [];
  for (const path of output.toString("utf8").split("\0")) {
    if (!path) continue;
    const absolutePath = resolve(repository, path);
    let metadata;
    try {
      metadata = lstatSync(absolutePath);
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw new Error("credential safeguard could not read the working tree");
    }
    if (metadata.isSymbolicLink()) {
      findings.push({ path, rule: "unscannable-symlink" });
      continue;
    }
    if (!metadata.isFile()) continue;
    if (metadata.size > MAX_TEXT_BYTES && !binaryExtensions.has(extension(path))) {
      findings.push({ path, rule: "unscannable-large-text" });
      continue;
    }
    try {
      findings.push(...scanBlob(path, readFileSync(absolutePath)));
    } catch {
      throw new Error("credential safeguard could not read the working tree");
    }
  }
  return findings;
}

function uniqueFindings(findings) {
  const seen = new Set();
  return findings.filter(({ path, rule }) => {
    const key = `${path}\0${rule}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function scanRepository(repository = process.cwd()) {
  return uniqueFindings([...scanIndex(repository), ...scanWorkingTree(repository)]);
}

export function formatFindings(findings) {
  return findings.map(({ path, rule }) => `${path.replace(/[\r\n\t]/g, "?")}: ${rule}`).join("\n");
}

function main() {
  let findings;
  try {
    findings = scanRepository();
  } catch {
    console.error("agent credential safeguard failed closed: repository snapshot unavailable");
    process.exitCode = 2;
    return;
  }
  if (findings.length > 0) {
    console.error("agent credential safeguard rejected the repository snapshot:");
    console.error(formatFindings(findings));
    console.error("matched values are intentionally redacted");
    process.exitCode = 1;
    return;
  }
  console.log("agent credential safeguard passed: Git index and working tree contain no high-confidence credential findings");
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  main();
}
