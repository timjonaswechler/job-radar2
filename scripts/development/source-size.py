#!/usr/bin/env python3
"""Report tracked Rust and TypeScript source size by production and test scope."""

from __future__ import annotations

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path
from typing import DefaultDict, Iterable, Mapping, Sequence, Set, Tuple


ROOT = Path(__file__).resolve().parents[2]
RUST_APP_ROOT = Path("src-tauri/src")
RUST_CRATES_ROOT = Path("src-tauri/crates")
RUST_APP_TEST_ROOT = Path("src-tauri/tests")
FRONTEND_ROOT = Path("src")


def tracked_files(patterns: Sequence[str]) -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "--", *patterns],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [Path(line) for line in result.stdout.splitlines() if line]


def mask_rust_comments_and_literals(source: str) -> str:
    """Keep Rust structure and newlines while masking comments and literals."""
    chars = list(source)
    i = 0
    block_depth = 0
    while i < len(source):
        if block_depth:
            if source.startswith("/*", i):
                chars[i : i + 2] = "  "
                block_depth += 1
                i += 2
            elif source.startswith("*/", i):
                chars[i : i + 2] = "  "
                block_depth -= 1
                i += 2
            else:
                if source[i] != "\n":
                    chars[i] = " "
                i += 1
            continue

        if source.startswith("//", i):
            end = source.find("\n", i + 2)
            end = len(source) if end == -1 else end
            chars[i:end] = " " * (end - i)
            i = end
            continue
        if source.startswith("/*", i):
            chars[i : i + 2] = "  "
            block_depth = 1
            i += 2
            continue

        raw = re.match(r"(?:br|r)(?P<hashes>#{0,255})\"", source[i:])
        if raw:
            delimiter = '"' + raw.group("hashes")
            end = source.find(delimiter, i + raw.end())
            end = len(source) if end == -1 else end + len(delimiter)
            for offset in range(i, end):
                if source[offset] != "\n":
                    chars[offset] = " "
            chars[i] = "L"
            i = end
            continue

        quote_start = i + 1 if source.startswith('b"', i) else i
        if quote_start < len(source) and source[quote_start] == '"':
            end = quote_start + 1
            while end < len(source):
                if source[end] == "\\":
                    end += 2
                elif source[end] == '"':
                    end += 1
                    break
                else:
                    end += 1
            for offset in range(i, min(end, len(source))):
                if source[offset] != "\n":
                    chars[offset] = " "
            chars[i] = "L"
            i = end
            continue

        if source[i] == "'" and i + 2 < len(source):
            char_match = re.match(r"'(?:\\.|[^\\'\n])'", source[i:])
            if char_match:
                end = i + char_match.end()
                chars[i:end] = "L" + " " * (end - i - 1)
                i = end
                continue
        i += 1
    return "".join(chars)


def matching_delimiter(source: str, start: int, opener: str, closer: str) -> int:
    depth = 0
    for offset in range(start, len(source)):
        if source[offset] == opener:
            depth += 1
        elif source[offset] == closer:
            depth -= 1
            if depth == 0:
                return offset + 1
    return len(source)


def skip_attributes(source: str, start: int) -> int:
    offset = start
    while True:
        match = re.match(r"\s*#\s*!?\s*\[", source[offset:])
        if not match:
            return offset + len(source[offset:]) - len(source[offset:].lstrip())
        bracket = offset + match.end() - 1
        offset = matching_delimiter(source, bracket, "[", "]")


def rust_item_end(source: str, start: int) -> int:
    parens = brackets = 0
    offset = start
    while offset < len(source):
        char = source[offset]
        if char == "(":
            parens += 1
        elif char == ")" and parens:
            parens -= 1
        elif char == "[":
            brackets += 1
        elif char == "]" and brackets:
            brackets -= 1
        elif parens == 0 and brackets == 0:
            if char == "{":
                return matching_delimiter(source, offset, "{", "}")
            if char in ";,":
                return offset + 1
        offset += 1
    return len(source)


def cfg_test_spans(source: str) -> list[Tuple[int, int]]:
    masked = mask_rust_comments_and_literals(source)
    spans: list[Tuple[int, int]] = []
    for match in re.finditer(r"#\s*\[\s*cfg\s*\(", masked):
        bracket = masked.rfind("[", match.start(), match.end())
        attribute_end = matching_delimiter(masked, bracket, "[", "]")
        if not re.search(r"\btest\b", masked[match.start() : attribute_end]):
            continue
        item_start = skip_attributes(masked, attribute_end)
        spans.append((match.start(), rust_item_end(masked, item_start)))
    return spans


def line_numbers_for_spans(source: str, spans: Iterable[Tuple[int, int]]) -> Set[int]:
    lines: Set[int] = set()
    for start, end in spans:
        start_line = source.count("\n", 0, start)
        end_line = source.count("\n", 0, max(start, end - 1))
        lines.update(range(start_line, end_line + 1))
    return lines


def nonempty_lines(source: str) -> Set[int]:
    return {index for index, line in enumerate(source.splitlines()) if line.strip()}


def cfg_test_modules(path: Path, source: str, spans: Sequence[Tuple[int, int]]) -> Set[Path]:
    modules: Set[Path] = set()
    for start, end in spans:
        declaration = source[start:end]
        match = re.search(
            r"#\s*\[\s*cfg\s*\([^]]*\btest\b[^]]*\)\s*\]"
            r"(?:\s*#\s*\[[^]]*\])*\s*(?:pub(?:\s*\([^)]*\))?\s+)?mod\s+(\w+)\s*;",
            declaration,
            re.DOTALL,
        )
        if not match:
            continue
        module = match.group(1)
        parent = path.parent if path.name in {"lib.rs", "main.rs", "mod.rs"} else path.parent / path.stem
        modules.add(parent / f"{module}.rs")
        modules.add(parent / module)
    return modules


def test_only_rust_files(
    files: Sequence[Path], source_data: Mapping[Path, Tuple[str, Sequence[Tuple[int, int]]]]
) -> Set[Path]:
    tracked = set(files)
    roots: Set[Path] = set()
    for path in files:
        source, spans = source_data[path]
        roots.update(cfg_test_modules(path, source, spans))

    test_files: Set[Path] = set()
    for root in roots:
        if root in tracked:
            test_files.add(root)
        test_files.update(path for path in files if root.suffix == "" and root in path.parents)
    return test_files


def rust_package(path: Path) -> str:
    if path.is_relative_to(RUST_CRATES_ROOT):
        return path.relative_to(RUST_CRATES_ROOT).parts[0]
    return "job-radar"


def rust_module(path: Path) -> str:
    if path.is_relative_to(RUST_CRATES_ROOT):
        relative = path.relative_to(RUST_CRATES_ROOT)
        package = relative.parts[0]
        source_relative = Path(*relative.parts[2:])
    else:
        package = "job-radar"
        source_relative = path.relative_to(RUST_APP_ROOT)
    first = source_relative.parts[0]
    module = Path(first).stem if len(source_relative.parts) == 1 else first
    return f"{package}/{module}"


def external_test_target(path: Path) -> str:
    if path.is_relative_to(RUST_APP_TEST_ROOT):
        relative = path.relative_to(RUST_APP_TEST_ROOT)
    else:
        parts = path.parts
        tests_index = parts.index("tests")
        relative = Path(*parts[tests_index + 1 :])
    first = relative.parts[0]
    return Path(first).stem


def print_table(headers: Sequence[str], rows: Sequence[Sequence[object]]) -> None:
    string_rows = [[str(value) for value in row] for row in rows]
    widths = [len(header) for header in headers]
    for row in string_rows:
        widths = [max(width, len(value)) for width, value in zip(widths, row)]
    print("  ".join(header.ljust(width) for header, width in zip(headers, widths)))
    print("  ".join("-" * width for width in widths))
    for row in string_rows:
        cells = [row[0].ljust(widths[0])]
        cells.extend(value.rjust(width) for value, width in zip(row[1:], widths[1:]))
        print("  ".join(cells))


def rust_report() -> None:
    source_files = tracked_files(
        [
            "src-tauri/src/**/*.rs",
            "src-tauri/src/*.rs",
            "src-tauri/crates/*/src/**/*.rs",
            "src-tauri/crates/*/src/*.rs",
        ]
    )
    external_files = tracked_files(
        [
            "src-tauri/tests/**/*.rs",
            "src-tauri/tests/*.rs",
            "src-tauri/crates/*/tests/**/*.rs",
            "src-tauri/crates/*/tests/*.rs",
        ]
    )
    source_data = {
        path: (
            source := (ROOT / path).read_text(encoding="utf-8"),
            cfg_test_spans(source),
        )
        for path in source_files
    }
    test_only = test_only_rust_files(source_files, source_data)
    packages: DefaultDict[str, list[int]] = defaultdict(lambda: [0, 0, 0])
    modules: DefaultDict[str, list[int]] = defaultdict(lambda: [0, 0])
    external_targets: DefaultDict[Tuple[str, str], int] = defaultdict(int)

    for path in source_files:
        source, spans = source_data[path]
        source_lines = nonempty_lines(source)
        inline_lines = (
            source_lines
            if path in test_only
            else source_lines & line_numbers_for_spans(source, spans)
        )
        production = len(source_lines - inline_lines)
        inline = len(inline_lines)
        packages[rust_package(path)][0] += production
        packages[rust_package(path)][1] += inline
        modules[rust_module(path)][0] += production
        modules[rust_module(path)][1] += inline

    for path in external_files:
        count = len(nonempty_lines((ROOT / path).read_text(encoding="utf-8")))
        package = rust_package(path)
        packages[package][2] += count
        external_targets[(package, external_test_target(path))] += count

    print("Rust packages")
    package_rows = []
    for package, counts in sorted(packages.items()):
        package_rows.append([package, *counts, sum(counts)])
    package_rows.append(
        [
            "TOTAL",
            *[sum(row[index] for row in package_rows) for index in range(1, 4)],
            sum(sum(row[1:4]) for row in package_rows),
        ]
    )
    print_table(
        ["Package", "Production", "Inline tests", "External tests", "Total"],
        package_rows,
    )

    print("\nRust top-level modules")
    module_rows = [
        [module, *counts, sum(counts)] for module, counts in sorted(modules.items())
    ]
    print_table(["Module", "Production", "Inline tests", "Total"], module_rows)

    print("\nRust external test targets")
    target_rows = [
        [f"{package}/{target}", count]
        for (package, target), count in sorted(external_targets.items())
    ]
    print_table(["Target", "Lines"], target_rows)


def is_frontend_test(path: Path) -> bool:
    relative = path.relative_to(FRONTEND_ROOT)
    return any(part in {"tests", "__tests__"} for part in relative.parts[:-1]) or bool(
        re.search(r"\.(?:test|spec)\.(?:ts|tsx)$", path.name)
    )


def frontend_area(path: Path) -> str:
    relative = path.relative_to(FRONTEND_ROOT)
    return relative.parts[0] if len(relative.parts) > 1 else "[root]"


def frontend_report() -> None:
    files = tracked_files(["src/**/*.ts", "src/**/*.tsx", "src/*.ts", "src/*.tsx"])
    areas: DefaultDict[str, list[int]] = defaultdict(lambda: [0, 0, 0, 0])
    for path in files:
        count = len(nonempty_lines((ROOT / path).read_text(encoding="utf-8")))
        test_offset = 2 if is_frontend_test(path) else 0
        extension_offset = 1 if path.suffix == ".tsx" else 0
        areas[frontend_area(path)][test_offset + extension_offset] += count

    print("\nTypeScript/React areas")
    rows = [[area, *counts, sum(counts)] for area, counts in sorted(areas.items())]
    rows.append(
        [
            "TOTAL",
            *[sum(row[index] for row in rows) for index in range(1, 5)],
            sum(sum(row[1:5]) for row in rows),
        ]
    )
    print_table(
        ["Area", "Prod .ts", "Prod .tsx", "Test .ts", "Test .tsx", "Total"],
        rows,
    )


def main() -> int:
    try:
        rust_report()
        frontend_report()
    except (OSError, subprocess.CalledProcessError) as error:
        print(f"source-size: {error}", file=sys.stderr)
        return 1
    print("\nCounts are non-empty physical lines in tracked source files; comments are included.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
