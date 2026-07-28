import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, isAbsolute, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

function maskCodeSpans(markdown) {
  const characters = [...markdown];
  for (let start = 0; start < characters.length; ) {
    if (characters[start] !== "`") {
      start += 1;
      continue;
    }
    let openingEnd = start;
    while (characters[openingEnd] === "`") openingEnd += 1;
    const delimiterLength = openingEnd - start;
    let closingStart = openingEnd;
    let closingEnd = openingEnd;
    while (closingStart < characters.length) {
      if (characters[closingStart] !== "`") {
        closingStart += 1;
        continue;
      }
      closingEnd = closingStart;
      while (characters[closingEnd] === "`") closingEnd += 1;
      if (closingEnd - closingStart === delimiterLength) break;
      closingStart = closingEnd;
    }
    if (closingStart >= characters.length) {
      start = openingEnd;
      continue;
    }
    for (let index = start; index < closingEnd; index += 1) {
      if (characters[index] !== "\n") characters[index] = " ";
    }
    start = closingEnd;
  }
  return characters.join("");
}

function visibleMarkdownLines(markdown) {
  const withoutComments = markdown.replace(/<!--[\s\S]*?-->/g, (comment) =>
    comment.replace(/[^\n]/g, " "),
  );
  let fence = null;
  let indentedBlock = false;
  let previousLineBlank = true;
  const withoutBlocks = withoutComments.split(/\r?\n/).map((line) => {
    const marker = line.match(/^ {0,3}(`{3,}|~{3,})/)?.[1] ?? null;
    if (fence !== null) {
      const afterMarker = marker === null ? line : line.slice(line.indexOf(marker) + marker.length);
      if (
        marker !== null &&
        marker[0] === fence[0] &&
        marker.length >= fence.length &&
        afterMarker.trim() === ""
      ) {
        fence = null;
      }
      return "";
    }
    if (marker !== null) {
      fence = marker;
      return "";
    }

    const blank = /^\s*$/.test(line);
    if (blank) {
      previousLineBlank = true;
      return "";
    }
    const indented = /^(?: {4}|\t)/.test(line);
    if (indented && (indentedBlock || previousLineBlank)) {
      indentedBlock = true;
      previousLineBlank = false;
      return "";
    }
    indentedBlock = false;
    previousLineBlank = false;
    return line;
  });
  return maskCodeSpans(withoutBlocks.join("\n")).split("\n");
}

function inlineDestinations(line) {
  const destinations = [];
  for (let start = 0; start < line.length; start += 1) {
    if (line[start] !== "]" || line[start + 1] !== "(") continue;
    let depth = 1;
    let escaped = false;
    let end = start + 2;
    for (; end < line.length; end += 1) {
      const character = line[end];
      if (escaped) {
        escaped = false;
        continue;
      }
      if (character === "\\") {
        escaped = true;
        continue;
      }
      if (character === "(") depth += 1;
      if (character === ")") depth -= 1;
      if (depth === 0) break;
    }
    if (depth === 0) {
      destinations.push(line.slice(start + 2, end));
      start = end;
    }
  }
  return destinations;
}

function destinationToken(rawDestination) {
  const trimmed = rawDestination.trim();
  if (trimmed.startsWith("<")) {
    const end = trimmed.indexOf(">");
    return end >= 0 ? trimmed.slice(1, end) : trimmed;
  }
  return trimmed.match(/^(?:\\.|\S)+/)?.[0]?.replace(/\\([ ()])/g, "$1") ?? "";
}

export function extractMarkdownLinks(markdown) {
  const links = [];
  const lines = visibleMarkdownLines(markdown);
  lines.forEach((line, index) => {
    for (const rawDestination of inlineDestinations(line)) {
      const destination = destinationToken(rawDestination);
      if (destination) links.push({ destination, line: index + 1 });
    }

    const reference = line.match(/^\s{0,3}\[[^\]]+\]:\s*(.+)$/);
    if (reference) {
      const destination = destinationToken(reference[1]);
      if (destination) links.push({ destination, line: index + 1 });
    }
  });
  return links;
}

function decode(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return null;
  }
}

function externalDestination(destination) {
  return (
    destination.startsWith("//") ||
    isAbsolute(destination) ||
    /^[a-z][a-z0-9+.-]*:/i.test(destination)
  );
}

function headingText(line) {
  const atx = line.match(/^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$/);
  return atx?.[1] ?? null;
}

function githubSlug(text) {
  return text
    .replace(/!?(?:\[([^\]]*)\])\([^)]*\)/g, "$1")
    .replace(/<[^>]+>/g, "")
    .replace(/[`*_~]/g, "")
    .trim()
    .toLocaleLowerCase("en-US")
    .replace(/[^\p{L}\p{N}_\- ]/gu, "")
    .replace(/\s+/g, "-");
}

function headingAnchors(markdown) {
  const anchors = new Set();
  const counts = new Map();
  const lines = visibleMarkdownLines(markdown);
  for (let index = 0; index < lines.length; index += 1) {
    let text = headingText(lines[index]);
    if (!text && index + 1 < lines.length && /^\s*(?:=+|-+)\s*$/.test(lines[index + 1])) {
      text = lines[index].trim();
    }
    if (!text) continue;
    const base = githubSlug(text);
    const count = counts.get(base) ?? 0;
    anchors.add(count === 0 ? base : `${base}-${count}`);
    counts.set(base, count + 1);
  }
  return anchors;
}

function repositoryRelative(root, absolutePath) {
  return relative(root, absolutePath).split(sep).join("/");
}

function targetFor(sourceAbsolute, pathPart) {
  return pathPart ? resolve(dirname(sourceAbsolute), pathPart) : sourceAbsolute;
}

export function checkMarkdownLinks(root, files) {
  const absoluteRoot = resolve(root);
  const findings = [];
  const markdownCache = new Map();

  for (const source of [...files].sort()) {
    const sourceAbsolute = resolve(absoluteRoot, source);
    const markdown = readFileSync(sourceAbsolute, "utf8");
    for (const { destination, line } of extractMarkdownLinks(markdown)) {
      if (externalDestination(destination)) continue;

      const hashIndex = destination.indexOf("#");
      const destinationPath = hashIndex >= 0 ? destination.slice(0, hashIndex) : destination;
      const rawFragment = hashIndex >= 0 ? destination.slice(hashIndex + 1) : "";
      const queryIndex = destinationPath.indexOf("?");
      const rawPath = queryIndex >= 0 ? destinationPath.slice(0, queryIndex) : destinationPath;
      const decodedPath = decode(rawPath);
      const decodedFragment = decode(rawFragment);
      if (decodedPath === null || decodedFragment === null) {
        findings.push({ source, line, destination, reason: "invalid URL encoding" });
        continue;
      }

      const target = targetFor(sourceAbsolute, decodedPath);
      const relativeTarget = relative(absoluteRoot, target);
      if (relativeTarget === ".." || relativeTarget.startsWith(`..${sep}`)) {
        findings.push({ source, line, destination, reason: "target escapes repository" });
        continue;
      }
      if (!existsSync(target)) {
        findings.push({ source, line, destination, reason: "target does not exist" });
        continue;
      }

      if (decodedFragment) {
        let markdownTarget = target;
        if (statSync(target).isDirectory()) markdownTarget = resolve(target, "README.md");
        if (extname(markdownTarget).toLowerCase() !== ".md") continue;
        if (!existsSync(markdownTarget)) {
          findings.push({
            source,
            line,
            destination,
            reason: "heading anchor does not exist",
          });
          continue;
        }
        const cacheKey = repositoryRelative(absoluteRoot, markdownTarget);
        let anchors = markdownCache.get(cacheKey);
        if (!anchors) {
          anchors = headingAnchors(readFileSync(markdownTarget, "utf8"));
          markdownCache.set(cacheKey, anchors);
        }
        if (!anchors.has(decodedFragment)) {
          findings.push({
            source,
            line,
            destination,
            reason: "heading anchor does not exist",
          });
        }
      }
    }
  }
  return findings;
}

export function formatMarkdownLinkFindings(findings) {
  return findings
    .map(({ source, line, destination, reason }) => `${source}:${line}: ${destination} (${reason})`)
    .join("\n");
}

export function trackedMarkdownFiles(root = process.cwd()) {
  const result = spawnSync("git", ["ls-files", "-z", "--", "*.md"], {
    cwd: root,
    encoding: "utf8",
  });
  if (result.status !== 0) throw new Error("could not enumerate tracked Markdown files");
  return result.stdout.split("\0").filter(Boolean);
}

function main() {
  const root = process.cwd();
  let files;
  let findings;
  try {
    files = trackedMarkdownFiles(root);
    findings = checkMarkdownLinks(root, files);
  } catch {
    console.error("markdown link check failed closed: repository documentation unavailable");
    process.exitCode = 2;
    return;
  }

  if (findings.length > 0) {
    console.error("markdown link check found invalid internal links:");
    console.error(formatMarkdownLinkFindings(findings));
    process.exitCode = 1;
    return;
  }
  console.log(`markdown link check passed: ${files.length} tracked Markdown files`);
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) main();
