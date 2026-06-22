#!/usr/bin/env python3
"""
Split a flat Rust module by copying complete top-level items into submodule files.

The script is intentionally syntax-light, but safer than line-number slicing:
- it discovers top-level Rust items,
- keeps directly attached attributes/doc comments with the item,
- finds item ends by balanced delimiters/semicolons,
- ignores braces in strings and comments,
- writes files from item names/selectors listed in a JSON plan.

Typical flow:

  # 1) Inspect exact selectors/spans from the real source file.
  scripts/rust-module-split.py list src-tauri/src/source_registry.rs

  # 2) Create a JSON skeleton and edit modules.todo into real groups.
  scripts/rust-module-split.py plan src-tauri/src/source_registry.rs > /tmp/source_registry.split.json

  # 3) Dry-run the split.
  scripts/rust-module-split.py split src-tauri/src/source_registry.rs \
    --plan /tmp/source_registry.split.json \
    --out-dir /tmp/source_registry-split

  # 4) Actually write the split files.
  scripts/rust-module-split.py split src-tauri/src/source_registry.rs \
    --plan /tmp/source_registry.split.json \
    --out-dir src-tauri/src/source_registry \
    --write

Plan format:

  {
    "modules": {
      "builtins": [
        "EmbeddedSourceRegistryDocument",
        "BUILTIN_SOURCE_PROFILE_JSON_FILES"
      ],
      "snapshot": [
        "SourceRegistrySnapshot",
        "impl SourceRegistrySnapshot"
      ]
    }
  }

Selectors accepted by split:
- non-impl items: their item name, e.g. "SourceDocument" or "load_snapshot"
- impl blocks: generated selector, e.g. "impl SourceRegistrySnapshot"
  or "impl Deserialize for SourceDocument"
"""

from __future__ import annotations

import argparse
import bisect
import json
import re
import shutil
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, Optional

IDENT = r"[A-Za-z_][A-Za-z0-9_]*"
VIS_RE = re.compile(r"pub(?:\s*\([^)]*\))?\s+")


@dataclass(frozen=True)
class RustItem:
    kind: str
    name: str
    selector: str
    visibility: Optional[str]
    start: int
    decl_start: int
    end: int
    line_start: int
    line_end: int
    text: str

    @property
    def is_exportable(self) -> bool:
        return self.visibility is not None and self.kind not in {"impl", "use"}


def line_offsets(text: str) -> list[int]:
    offsets = [0]
    for match in re.finditer("\n", text):
        offsets.append(match.end())
    return offsets


def line_no(offsets: list[int], offset: int) -> int:
    return bisect.bisect_right(offsets, offset)


def skip_line_comment(text: str, i: int) -> int:
    end = text.find("\n", i + 2)
    return len(text) if end == -1 else end + 1


def skip_block_comment(text: str, i: int) -> int:
    depth = 1
    i += 2
    while i < len(text) and depth:
        if text.startswith("/*", i):
            depth += 1
            i += 2
        elif text.startswith("*/", i):
            depth -= 1
            i += 2
        else:
            i += 1
    return i


def raw_string_end(text: str, i: int) -> Optional[int]:
    """Return end offset if a Rust raw string starts at i, else None."""
    j = i
    if text.startswith("br", i):
        j = i + 2
    elif text.startswith("r", i):
        j = i + 1
    else:
        return None

    hashes = 0
    while j + hashes < len(text) and text[j + hashes] == "#":
        hashes += 1
    quote = j + hashes
    if quote >= len(text) or text[quote] != '"':
        return None

    delimiter = '"' + ("#" * hashes)
    end = text.find(delimiter, quote + 1)
    return len(text) if end == -1 else end + len(delimiter)


def normal_string_end(text: str, i: int) -> Optional[int]:
    """Return end offset if a normal or byte string starts at i, else None."""
    if text.startswith('b"', i):
        i += 1
    elif i >= len(text) or text[i] != '"':
        return None

    i += 1
    while i < len(text):
        c = text[i]
        if c == "\\":
            i += 2
            continue
        if c == '"':
            return i + 1
        i += 1
    return len(text)


def char_literal_end(text: str, i: int) -> Optional[int]:
    """Return end offset for simple Rust char literals; avoid lifetimes like 'de."""
    if i >= len(text) or text[i] != "'" or i + 2 >= len(text):
        return None

    # Escaped char: '\n', '\'', '\u{...}', etc. Find the next quote nearby.
    if text[i + 1] == "\\":
        limit = min(len(text), i + 32)
        j = i + 2
        while j < limit and text[j] != "\n":
            if text[j] == "'":
                return j + 1
            j += 1
        return None

    # One-codepoint char like 'x' or '{'. Lifetimes like 'de do not match.
    if text[i + 1] != "\n" and text[i + 2] == "'":
        return i + 3

    return None


def skip_ignored(text: str, i: int) -> int:
    """Skip comments/strings/chars starting at i. Return i unchanged if none."""
    if text.startswith("//", i):
        return skip_line_comment(text, i)
    if text.startswith("/*", i):
        return skip_block_comment(text, i)

    end = raw_string_end(text, i)
    if end is not None:
        return end

    end = normal_string_end(text, i)
    if end is not None:
        return end

    end = char_literal_end(text, i)
    if end is not None:
        return end

    return i


def find_semicolon_end(text: str, start: int) -> int:
    paren = bracket = brace = 0
    i = start
    while i < len(text):
        skipped = skip_ignored(text, i)
        if skipped != i:
            i = skipped
            continue

        c = text[i]
        if c == "(":
            paren += 1
        elif c == ")" and paren:
            paren -= 1
        elif c == "[":
            bracket += 1
        elif c == "]" and bracket:
            bracket -= 1
        elif c == "{":
            brace += 1
        elif c == "}" and brace:
            brace -= 1
        elif c == ";" and paren == 0 and bracket == 0 and brace == 0:
            return i + 1
        i += 1
    raise ValueError("could not find top-level semicolon")


def find_matching_brace(text: str, open_brace: int) -> int:
    depth = 0
    i = open_brace
    while i < len(text):
        skipped = skip_ignored(text, i)
        if skipped != i:
            i = skipped
            continue

        c = text[i]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    raise ValueError("could not find matching closing brace")


def find_body_or_semicolon_end(text: str, start: int) -> int:
    paren = bracket = 0
    i = start
    while i < len(text):
        skipped = skip_ignored(text, i)
        if skipped != i:
            i = skipped
            continue

        c = text[i]
        if c == "(":
            paren += 1
        elif c == ")" and paren:
            paren -= 1
        elif c == "[":
            bracket += 1
        elif c == "]" and bracket:
            bracket -= 1
        elif c == ";" and paren == 0 and bracket == 0:
            return i + 1
        elif c == "{" and paren == 0 and bracket == 0:
            close = find_matching_brace(text, i)
            end = close + 1
            # macro_rules! blocks are commonly followed by a semicolon. Including an
            # immediate trailing semicolon is harmless for ordinary braced items too.
            j = end
            while j < len(text) and text[j] in " \t\r\n":
                j += 1
            if j < len(text) and text[j] == ";":
                end = j + 1
            return end
        i += 1
    raise ValueError("could not find item body or semicolon")


def strip_visibility(decl: str) -> tuple[Optional[str], str]:
    stripped = decl.lstrip()
    match = re.match(r"pub(?:\s*\([^)]*\))?", stripped)
    if not match:
        return None, stripped

    visibility = re.sub(r"\s+", "", match.group(0))
    rest = stripped[match.end() :].lstrip()
    if not rest:
        return None, stripped
    return visibility, rest


def declaration_snippet(text: str, start: int, max_chars: int = 1200) -> str:
    return text[start : min(len(text), start + max_chars)]


def parse_item_decl(text: str, decl_start: int) -> Optional[tuple[str, str, Optional[str]]]:
    snippet = declaration_snippet(text, decl_start)
    visibility, rest = strip_visibility(snippet)
    compact = re.sub(r"\s+", " ", rest).strip()

    if compact.startswith("use "):
        return "use", compact[:80], visibility

    macro = re.match(rf"macro_rules!\s+({IDENT})", compact)
    if macro:
        return "macro_rules", macro.group(1), visibility

    if compact.startswith("impl"):
        return "impl", "impl", visibility

    fn_match = re.match(
        rf"(?:(?:async|unsafe|const)\s+)*(?:extern\s+\"[^\"]+\"\s+)?fn\s+({IDENT})",
        compact,
    )
    if fn_match:
        return "fn", fn_match.group(1), visibility

    for kind in ("type", "const", "static", "struct", "enum", "trait", "mod"):
        match = re.match(rf"{kind}\s+({IDENT})", compact)
        if match:
            return kind, match.group(1), visibility

    return None


def impl_selector_from_block(block: str) -> str:
    # Header before the first body brace, without attributes/docs.
    lines = []
    for line in block.splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or stripped.startswith("///"):
            continue
        lines.append(stripped)
        if "{" in stripped:
            break
    header = re.sub(r"\s+", " ", " ".join(lines))
    header = header.split("{", 1)[0].strip()
    header = re.sub(r"^impl\s*<[^>]*>\s*", "impl ", header)

    if " for " in header:
        before, after = header.split(" for ", 1)
        trait_name = before.removeprefix("impl ").strip().split("<", 1)[0]
        target = after.strip().split()[0].split("<", 1)[0]
        return f"impl {trait_name} for {target}"

    target = header.removeprefix("impl").strip().split()[0].split("<", 1)[0]
    return f"impl {target}"


def item_end(text: str, decl_start: int, kind: str) -> int:
    if kind in {"use", "type", "const", "static"}:
        return find_semicolon_end(text, decl_start)
    return find_body_or_semicolon_end(text, decl_start)


def starts_attached_prefix(stripped_line: str) -> bool:
    return (
        stripped_line.startswith("#")
        or stripped_line.startswith("///")
        or stripped_line.startswith("//!")
        or stripped_line.startswith("/**")
        or stripped_line.startswith("/*!")
    )


def scan_items(text: str) -> list[RustItem]:
    offsets = line_offsets(text)
    lines = text.splitlines(keepends=True)
    items: list[RustItem] = []
    line_index = 0
    pending_start: Optional[int] = None

    while line_index < len(lines):
        start = offsets[line_index]
        line = lines[line_index]
        stripped = line.lstrip()

        if not stripped.strip():
            pending_start = None
            line_index += 1
            continue

        if starts_attached_prefix(stripped):
            if pending_start is None:
                pending_start = start
            line_index += 1
            continue

        parsed = parse_item_decl(text, start)
        if parsed is None:
            pending_start = None
            line_index += 1
            continue

        kind, name, visibility = parsed
        item_start = pending_start if pending_start is not None else start
        end = item_end(text, start, kind)
        block = text[item_start:end]
        selector = impl_selector_from_block(block) if kind == "impl" else name
        item = RustItem(
            kind=kind,
            name=name if kind != "impl" else selector.removeprefix("impl "),
            selector=selector,
            visibility=visibility,
            start=item_start,
            decl_start=start,
            end=end,
            line_start=line_no(offsets, item_start),
            line_end=line_no(offsets, max(item_start, end - 1)),
            text=block,
        )
        items.append(item)

        line_index = bisect.bisect_right(offsets, max(item_start, end - 1))
        pending_start = None

    return items


def first_non_use_start(items: Iterable[RustItem], text_len: int) -> int:
    for item in items:
        if item.kind != "use":
            return item.start
    return text_len


def render_list(items: list[RustItem]) -> str:
    rows = []
    for item in items:
        if item.kind == "use":
            continue
        visibility = item.visibility or "-"
        rows.append(
            f"{item.selector:<48} {item.kind:<11} {visibility:<10} {item.line_start:>5}-{item.line_end:<5}"
        )
    header = f"{'selector':<48} {'kind':<11} {'visibility':<10} {'lines'}"
    return "\n".join([header, "-" * len(header), *rows])


def item_to_json(item: RustItem) -> dict[str, object]:
    return {
        "selector": item.selector,
        "kind": item.kind,
        "name": item.name,
        "visibility": item.visibility,
        "lineStart": item.line_start,
        "lineEnd": item.line_end,
    }


def plan_skeleton(source: Path, items: list[RustItem]) -> dict[str, object]:
    selectors = [item.selector for item in items if item.kind != "use"]
    return {
        "source": str(source),
        "modules": {
            "todo": selectors,
        },
    }


def load_plan(path: Path) -> dict[str, list[str]]:
    raw = json.loads(path.read_text())
    modules = raw.get("modules")
    if not isinstance(modules, dict):
        raise SystemExit("plan must contain an object field named 'modules'")

    normalized: dict[str, list[str]] = {}
    for module, selectors in modules.items():
        if not isinstance(module, str) or not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", module):
            raise SystemExit(f"invalid module name in plan: {module!r}")
        if not isinstance(selectors, list) or not all(isinstance(s, str) for s in selectors):
            raise SystemExit(f"module {module!r} must contain a list of selectors")
        normalized[module] = selectors
    return normalized


def selector_index(items: list[RustItem]) -> dict[str, RustItem]:
    index: dict[str, RustItem] = {}
    ambiguous: set[str] = set()
    for item in items:
        if item.kind == "use":
            continue
        # Impl blocks intentionally match only their generated selector
        # (e.g. `impl SourceRegistrySnapshot`), otherwise their target type
        # name would collide with the actual struct/enum item.
        keys = {item.selector} if item.kind == "impl" else {item.selector, item.name}
        for key in keys:
            if key in index and index[key] != item:
                ambiguous.add(key)
            else:
                index[key] = item
    for key in ambiguous:
        index.pop(key, None)
    return index


def resolve_plan_items(modules: dict[str, list[str]], items: list[RustItem]) -> dict[str, list[RustItem]]:
    index = selector_index(items)
    resolved: dict[str, list[RustItem]] = {}
    used: dict[RustItem, str] = {}

    for module, selectors in modules.items():
        module_items = []
        for selector in selectors:
            item = index.get(selector)
            if item is None:
                raise SystemExit(f"selector {selector!r} was not found or is ambiguous")
            if item in used:
                raise SystemExit(
                    f"selector {selector!r} is assigned to both {used[item]!r} and {module!r}"
                )
            used[item] = module
            module_items.append(item)
        resolved[module] = module_items
    return resolved


def reexport_groups(module_items: list[RustItem]) -> dict[str, list[str]]:
    groups: dict[str, list[str]] = {}
    for item in module_items:
        if not item.is_exportable:
            continue
        visibility = item.visibility or ""
        groups.setdefault(visibility, []).append(item.name)
    return groups


def render_mod_rs(resolved: dict[str, list[RustItem]]) -> str:
    lines: list[str] = []
    for module in resolved:
        if module == "tests":
            lines.append("#[cfg(test)]")
        lines.append(f"mod {module};")
    lines.append("")

    for module, module_items in resolved.items():
        if module == "tests":
            continue
        for visibility, names in reexport_groups(module_items).items():
            names = sorted(set(names))
            if not names:
                continue
            prefix = "pub use" if visibility == "pub" else f"{visibility} use"
            if len(names) == 1:
                lines.append(f"{prefix} {module}::{names[0]};")
            else:
                joined = ", ".join(names)
                lines.append(f"{prefix} {module}::{{{joined}}};")
    return "\n".join(lines).rstrip() + "\n"


def unwrap_single_test_mod(module: str, items: list[RustItem]) -> Optional[str]:
    if module != "tests" or len(items) != 1:
        return None
    item = items[0]
    if item.kind != "mod" or item.name != "tests":
        return None
    open_brace = item.text.find("{")
    close_brace = item.text.rfind("}")
    if open_brace == -1 or close_brace == -1 or close_brace <= open_brace:
        return None
    inner = item.text[open_brace + 1 : close_brace]
    # Drop exactly one leading newline introduced by `mod tests {` formatting.
    if inner.startswith("\n"):
        inner = inner[1:]
    return inner.rstrip() + "\n"


def render_module_file(
    module: str,
    items: list[RustItem],
    preamble: str,
    preamble_mode: str,
    super_glob: bool,
    unwrap_tests: bool,
) -> str:
    if unwrap_tests:
        unwrapped = unwrap_single_test_mod(module, items)
        if unwrapped is not None:
            return unwrapped

    parts: list[str] = []
    if preamble_mode == "copy" and preamble.strip():
        parts.append(preamble.rstrip())
    if super_glob:
        parts.append("use super::*;")
    parts.extend(item.text.rstrip() for item in items)
    return "\n\n".join(part for part in parts if part).rstrip() + "\n"


def split_module(args: argparse.Namespace) -> None:
    source = Path(args.source)
    text = source.read_text()
    items = scan_items(text)
    modules = load_plan(Path(args.plan))
    resolved = resolve_plan_items(modules, items)

    missing = [item.selector for item in items if item.kind != "use" and all(item not in ms for ms in resolved.values())]
    if missing and not args.allow_unassigned:
        raise SystemExit(
            "plan does not assign all top-level items. Missing selectors:\n  "
            + "\n  ".join(missing)
        )

    out_dir = Path(args.out_dir)
    preamble = text[: first_non_use_start(items, len(text))]
    files: dict[Path, str] = {
        out_dir / "mod.rs": render_mod_rs(resolved),
    }
    for module, module_items in resolved.items():
        files[out_dir / f"{module}.rs"] = render_module_file(
            module,
            module_items,
            preamble,
            args.preamble,
            args.super_glob,
            not args.no_unwrap_tests,
        )

    if not args.write:
        print(f"dry-run: would write {len(files)} files under {out_dir}")
        for path, content in files.items():
            print(f"  {path} ({content.count(chr(10))} lines)")
        return

    if out_dir.exists():
        if not args.force:
            raise SystemExit(f"output directory exists: {out_dir} (use --force to replace)")
        shutil.rmtree(out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    for path, content in files.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    print(f"wrote {len(files)} files under {out_dir}")
    if args.replace_source:
        source.unlink()
        print(f"removed original source file: {source}")


def main(argv: Optional[list[str]] = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = parser.add_subparsers(dest="command", required=True)

    list_parser = sub.add_parser("list", help="list top-level item selectors and line spans")
    list_parser.add_argument("source")
    list_parser.add_argument("--json", action="store_true", help="emit JSON instead of a table")

    plan_parser = sub.add_parser("plan", help="emit a JSON split-plan skeleton")
    plan_parser.add_argument("source")

    split_parser = sub.add_parser("split", help="copy planned top-level items into submodule files")
    split_parser.add_argument("source")
    split_parser.add_argument("--plan", required=True, help="JSON plan path")
    split_parser.add_argument("--out-dir", required=True, help="directory for generated module files")
    split_parser.add_argument("--write", action="store_true", help="actually write files; default is dry-run")
    split_parser.add_argument("--force", action="store_true", help="replace an existing output directory")
    split_parser.add_argument(
        "--replace-source",
        action="store_true",
        help="delete the original flat .rs file after writing the folder module",
    )
    split_parser.add_argument(
        "--allow-unassigned",
        action="store_true",
        help="allow plan to omit some top-level items",
    )
    split_parser.add_argument(
        "--preamble",
        choices=("copy", "none"),
        default="copy",
        help="copy the original preamble/imports into each generated file",
    )
    split_parser.add_argument(
        "--super-glob",
        action="store_true",
        help="add `use super::*;` to generated non-test module files",
    )
    split_parser.add_argument(
        "--no-unwrap-tests",
        action="store_true",
        help="keep `mod tests { ... }` wrapped when module name is tests",
    )

    args = parser.parse_args(argv)
    source = Path(args.source)
    text = source.read_text()
    items = scan_items(text)

    if args.command == "list":
        if args.json:
            print(json.dumps([item_to_json(item) for item in items if item.kind != "use"], indent=2))
        else:
            print(render_list(items))
        return 0

    if args.command == "plan":
        print(json.dumps(plan_skeleton(source, items), indent=2))
        return 0

    if args.command == "split":
        split_module(args)
        return 0

    parser.error(f"unknown command: {args.command}")
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
