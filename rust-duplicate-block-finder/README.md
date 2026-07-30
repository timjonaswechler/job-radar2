# Rust Duplicate Block Finder

This CLI parses Rust with `syn` and creates review queues for exactly normalized
and structurally similar functions, plus exactly normalized `{ ... }` blocks.
It finds likely copy/paste or AI-generated redundancy; it does **not** prove
behavioral equivalence.

## Run

```bash
cargo run --release -- ../src-tauri \
  --min-lines 5 \
  --min-chars 80 \
  --function-min-lines 5 \
  --function-min-chars 80 \
  --similarity-threshold 0.80 \
  --max-similar-pairs 10000 \
  --output duplicate-report
```

`--similarity-threshold` accepts `0.0` through `1.0` and defaults to `0.80`.
`--max-similar-pairs` (default `10000`) deterministically keeps only the highest
scoring pairs. `--no-similarity` skips feature extraction and all pair comparison;
it also removes a stale `similar-functions.csv` from the selected output folder.
This option is useful when only the inexpensive exact reports are needed.

The block thresholds (`--min-lines` and `--min-chars`) control which blocks are
recorded. The separate function thresholds control eligibility for both
`duplicate-functions.csv` and `similar-functions.csv`. Short getters and setters
therefore remain in `functions.csv` by default but are not candidates. Pass
`--exclude-small-functions-from-inventory` to remove them from the inventory too.

By default, `.git`, `target`, `node_modules`, `.idea`, `.vscode`, `tests`,
`benches`, and `examples` are skipped. Add `--include-tests` to include the last
three.

## Exact normalization

Only function parameter names and local bindings are normalized to stable names
such as `LOCAL_0`. Member identifiers after `.` or `::` are preserved even when
they happen to have the same spelling as a local binding. Field names, method
names, called function names, macro names, type/variant names, and Rust keywords
remain distinguishable.

Numeric, string, and character literals become `LIT` for the exact structural
fingerprint. Their original spelling is nevertheless retained in similarity
diagnostics.

An exact function match requires the normalized signature (with the declared
function name removed) and normalized body to match. The signature includes the
receiver, parameter count and types, return type, `async`, `unsafe`, `const`, ABI,
generics, and `where` clauses. Thus renamed copies can match, while equal bodies
with different return types do not.

## Similarity score

Similarity is based on precomputed normalized Rust-token features, not character
or raw-text edit distance. The named weights in the implementation are:

- **55% body structure:** Jaccard similarity of normalized token trigrams
- **20% relevant identifiers:** Jaccard similarity of fields, calls, methods,
  macros, and type/variant-like identifiers; parameters and locals are excluded
- **15% normalized signature:** Jaccard similarity of normalized signature token
  trigrams, including all signature details listed above
- **10% control flow:** multiset overlap for `if`, `else`, `match`, `for`,
  `while`, `loop`, `return`, `break`, `continue`, `await`, and `?`

Literal similarity is deliberately diagnostic-only: `literal_similarity_percent`,
`literals_only_left`, and `literals_only_right` expose constant differences even
when normalized structure is 100%. Identifier differences are similarly exposed
in `identifiers_only_left` and `identifiers_only_right`.

Exact combined fingerprints receive 100% and `match_kind=exact_normalized`.
Non-exact pairs meeting the configured threshold receive
`match_kind=similar_normalized`.

Suggested interpretation:

- **100%:** exact normalized structure (literal values may still differ)
- **95–99%:** very likely refactoring candidate
- **90–95%:** strong similarity; inspect manually
- **80–90%:** possible shared abstraction
- **below 80%:** omitted by default

A high score only prioritizes review. Never merge functions automatically:
different side effects, domain responsibilities, types, and literal values can
make similar code intentionally distinct.

## Performance

Only functions passing `--function-min-lines` and `--function-min-chars` receive
similarity features. Tokenization, trigrams, identifier/literal sets, and control
counts are computed once before the pair loop. Before set intersections, a safe
upper bound is calculated from the feature-set sizes for every weighted score
component; a pair is skipped only when it cannot reach the configured threshold.
The result list is periodically trimmed using the unrounded score and finally
capped by `--max-similar-pairs`. These measures avoid repeated parsing, reject
many incompatible pairs cheaply, and bound report memory and output size.

## Reports

- `functions.csv`: function inventory, including small functions by default
- `blocks.csv`: all blocks passing block thresholds
- `duplicate-functions.csv`: relevant exact function groups
- `duplicate-blocks.csv`: exact block groups
- `similar-functions.csv`: deterministic function pairs at or above the threshold
- `errors.txt`: unreadable or unparsable files, when present

`similar-functions.csv` is sorted by `similarity_percent` descending, then by
left/right file, line, and function ID. Each unordered pair appears at most once,
never compares a function with itself, and receives a stable display ID such as
`SIM-0001`. It includes overall/body/signature/identifier/control-flow/literal
percentages, both function locations and IDs, match kind, comparison basis, and
identifier/literal differences. Pair rows are intentionally not transitively
grouped: similarity between A/B and B/C says nothing about A/C.

Exact duplicate reports remain grouped by `group_id` (`FN-0001`, `BLK-0001`, …).
Their `comparison_basis`, signature/body hashes, relevance, nesting information,
and exact `match_kind` retain their previous meaning. `duplicate_count = 0` in
`functions.csv` means threshold-excluded; `1` means eligible without an exact
match; only counts above one appear in exact duplicate reports.

## Limits

The analyzer compares syntactic token features, not semantics, data flow, or
mathematical equivalence. Shingle overlap can miss reordered equivalents and can
rank boilerplate highly. Literal normalization can surface functions whose
constants carry different business meaning. Generated code and broad framework
patterns may add noise. Treat every row as a manual review candidate and verify
side effects, error behavior, ownership/lifetimes, domain boundaries, and values
before refactoring.
