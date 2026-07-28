import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  checkMarkdownLinks,
  extractMarkdownLinks,
  formatMarkdownLinkFindings,
} from "./markdown-links.mjs";

const extracted = extractMarkdownLinks(`
[inline](guide/start.md#first-step)
[reference]: reference.md
Paragraph text
    [paragraph continuation](continuation.md)
\`[inline code](ignored-inline.md)\`
\`multiline code
[ignored](ignored-multiline.md)\`

    [indented code](ignored-indented.md)
\`\`[unequal delimiters](unequal.md)\`\`\`
<!-- [commented out](ignored-comment.md) -->

\`\`\`\`md
[ignored](missing.md)
\`\`\`
[still ignored](also-missing.md)
    \`\`\`\`
[ignored after indented marker](indented-marker.md)
\`\`\`\`
`);
assert.deepEqual(
  extracted.map(({ destination, line }) => ({ destination, line })),
  [
    { destination: "guide/start.md#first-step", line: 2 },
    { destination: "reference.md", line: 3 },
    { destination: "continuation.md", line: 5 },
    { destination: "unequal.md", line: 11 },
  ],
);

const root = mkdtempSync(join(tmpdir(), "job-radar-markdown-links-"));
try {
  mkdirSync(join(root, "docs", "guide"), { recursive: true });
  mkdirSync(join(root, "docs", "empty"), { recursive: true });
  writeFileSync(
    join(root, "docs", "index.md"),
    [
      "# Portal",
      "",
      "[Guide](guide/start.md#first-step)",
      "[Directory](guide/)",
      "[Same document](#portal)",
      "[External](https://example.test/missing.md)",
      "[Wrong-case anchor](#PORTAL)",
      "[Directory anchor](empty/#missing)",
      "[Missing](missing.md)",
      "[Missing anchor](guide/start.md#unknown)",
      "",
    ].join("\n"),
  );
  writeFileSync(
    join(root, "docs", "guide", "start.md"),
    "# Start\n\n## First step\n",
  );

  const findings = checkMarkdownLinks(root, [
    "docs/index.md",
    "docs/guide/start.md",
  ]);
  assert.deepEqual(
    findings.map(({ source, line, destination, reason }) => ({
      source,
      line,
      destination,
      reason,
    })),
    [
      {
        source: "docs/index.md",
        line: 7,
        destination: "#PORTAL",
        reason: "heading anchor does not exist",
      },
      {
        source: "docs/index.md",
        line: 8,
        destination: "empty/#missing",
        reason: "heading anchor does not exist",
      },
      {
        source: "docs/index.md",
        line: 9,
        destination: "missing.md",
        reason: "target does not exist",
      },
      {
        source: "docs/index.md",
        line: 10,
        destination: "guide/start.md#unknown",
        reason: "heading anchor does not exist",
      },
    ],
  );
  assert.equal(
    formatMarkdownLinkFindings(findings),
    [
      "docs/index.md:7: #PORTAL (heading anchor does not exist)",
      "docs/index.md:8: empty/#missing (heading anchor does not exist)",
      "docs/index.md:9: missing.md (target does not exist)",
      "docs/index.md:10: guide/start.md#unknown (heading anchor does not exist)",
    ].join("\n"),
  );
} finally {
  rmSync(root, { recursive: true, force: true });
}

console.log("markdown link checker self-tests passed");
