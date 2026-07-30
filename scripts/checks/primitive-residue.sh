#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
cd "$ROOT"

# Frozen union. Each expression runs independently so overlapping expressions and repeated
# same-line hits remain separate evidence. Stable IDs make the matched obligation explicit.
PATTERNS=(
  'parse_text:Parse::Text'
  'json_text:"text"'
  'browser_eval:BrowserInteraction::Eval'
  'json_eval:"eval"'
  'old_browser_client:ProfileBrowser(Client|Fetch)|ManagedProfileBrowserClient|UnavailableProfileBrowserClient|BoxedProfileBrowserFuture|render_with_context|BrowserRuntimeRender(Request|Response|Error)|render_page_html_with_actions_and_context|StaticBrowserClient|FixtureProfileBrowserClient|CancellationAwareBrowser|ManagedProfileBrowserClient::new|_with_clients|_with_clients_and_context|_with_fetcher'
  'old_detection:inputUrlPatterns|httpChecks|browserProbes|input_url_patterns|http_checks|browser_probes|ProfileDetectionDocument|InputUrlPattern|DetectionHttpCheck|DetectionBrowserProbe|DetectionBrowserInteraction|DetectionHttp(Client|Response|Error)|BoxedDetectionHttpFuture|NoopDetectionHttpClient|ReqwestDetectionHttpClient|match_input_url_patterns|evaluate_http_checks|evaluate_browser_probes|build_source_config|build_source_proposal|detect_source_proposal_with_(http_client|clients)|mutable.*(capture|config)|aggregate.*detection|proposal.*builder'
  'registration_residue:rejection.?only|unsupported.*registr|no.?op.*registr|clone_(parse|select)|compile_(fetch|pagination)|raw.*(plan|dispatch|fallback)|PrimitiveDescriptor|Compiled[A-Za-z0-9_]*Descriptor|register.*primitive|duplicate.*(family|descriptor|registr)|registry.*(parse|select|transform|regex|capture|fetch|pagination|accept|execute)'
  'product_dispatch:provider.*dispatch|host.*dispatch|profile.*dispatch|source.*dispatch'
  'browser_removed:execute_script|mutate_dom|login_flow|captcha_bypass'
  'transform_alias:normalizeWhitespace|htmlToText|urlDecode|slugToTitle|toString'
  'acceptance_removed:maxErrorRatio'
  'retry:Retry|retry'
  'pacing:Pacing|pacing'
  'rate_limit:rate.?limit|RateLimit'
  'retry_after:Retry-After|retry_after'
  'bot:Bot|bot.?detect'
  'predicate_all_any_none:Predicate::(All|Any|None)|"(all|any|none)"'
  'predicate_composition:negat|count requirement|count_requirement|minimumCount|maximumCount'
  'arbitrary_execution:execute_script|plugin|shell|JavaScript|javascript|login_flow|captcha_bypass'
  'browser_byte_dimension:maxBrowserRenderedBytes|browser_rendered_bytes'
  'http_byte_dimension:maxResponseBytes|response_bytes'
)

TMP=$(mktemp)
EXPECTED=$(mktemp)
trap 'rm -f "$TMP" "$EXPECTED"' EXIT
: > "$TMP"
for spec in "${PATTERNS[@]}"; do
  id=${spec%%:*}
  expression=${spec#*:}
  PATTERN_ID="$id" PATTERN_EXPR="$expression" python3 <<'PY' >> "$TMP"
import base64, json, os, subprocess
from pathlib import Path

roots = ["src-tauri/src", "src-tauri/tests", "src-tauri/crates/agent", "src-tauri/resources", "src", "docs"]
extensions = {".rs", ".json", ".ts", ".tsx", ".md"}
tracked = subprocess.run(
    ["git", "ls-files", "--", *roots],
    stdout=subprocess.PIPE,
    check=True,
).stdout.decode().splitlines()
paths = [path for path in tracked if Path(path).suffix in extensions and Path(path).is_file()]
# Workspace packages may be unstaged while this check runs locally; their source and tests are always scanned.
for package in ("source-profile-dsl", "search-resolution"):
    for directory, _, files in os.walk(f"src-tauri/crates/{package}"):
        for name in files:
            path = os.path.join(directory, name)
            if Path(name).suffix in extensions:
                paths.append(path)

def field(value):
    if "text" in value:
        return value["text"]
    return base64.b64decode(value["bytes"]).decode("utf-8", "surrogateescape")

def escaped(value):
    # Preserve arbitrary Unix path bytes (including invalid UTF-8) in one unambiguous line.
    raw = value.encode("utf-8", "surrogateescape")
    return "".join(
        chr(byte) if 0x20 <= byte < 0x7f and byte not in (ord("%"), ord(":"))
        else f"%{byte:02X}"
        for byte in raw
    )

pattern_id = os.environ["PATTERN_ID"]
expression = os.environ["PATTERN_EXPR"]
for offset in range(0, len(paths), 200):
    command = ["rg", "--json", "--no-messages", "-e", expression, *paths[offset:offset + 200]]
    result = subprocess.run(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
    if result.returncode not in (0, 1):
        raise SystemExit(result.stderr.decode("utf-8", "replace"))
    for raw in result.stdout.splitlines():
        event = json.loads(raw)
        if event["type"] != "match":
            continue
        data = event["data"]
        path = escaped(field(data["path"]))
        line = data["line_number"]
        for match in data["submatches"]:
            token = escaped(field(match["match"]))
            column = match["start"] + 1
            print(f"{path}:{line}:{column}:{pattern_id}:{token}")
PY
done
# The registry file itself is classified even when it contains no searchable behavior token.
printf '%s\n' 'src-tauri/crates/source-profile-dsl/src/profile_dsl/primitives/mod.rs:1:1:registry_file:primitives/mod.rs' >> "$TMP"
LC_ALL=C sort "$TMP" -o "$TMP"

if [[ ${1:-} == --emit ]]; then
  cat "$TMP"
  exit 0
fi

MANIFEST=${PRIMITIVE_RESIDUE_MANIFEST:-src-tauri/crates/source-profile-dsl/tests/fixtures/primitive_completeness/primitive-residue-classification.txt}
FROZEN_MANIFEST_SHA256='6cb59dbf7cc62def79de0b5d22650f9de7f81784a75f0866b346ba18c51e351f'
if [[ ${PRIMITIVE_RESIDUE_MANIFEST:-} == '' ]]; then
  actual_sha=$(shasum -a 256 "$MANIFEST" | awk '{print $1}')
  if [[ "$actual_sha" != "$FROZEN_MANIFEST_SHA256" ]]; then
    echo "Primitive residue classifications changed without reviewed SHA update: $actual_sha" >&2
    exit 1
  fi
fi
awk -F '\t' '!/^#/ && NF { print $1 }' "$MANIFEST" | LC_ALL=C sort > "$EXPECTED"
if ! diff -u "$EXPECTED" "$TMP"; then
  echo 'Primitive residue changed. Classify every path:line:column:pattern:token hit.' >&2
  exit 1
fi

ALLOWED='^(historical_documentation|active_contract_documentation|explicit_negative_or_contract_test|g02_removed_key_metadata|unrelated_agent_provider_retry|unrelated_browser_install_or_admin_retry|unrelated_frontend_ui_identifier|active_profile_contract|final_implementation_or_interface)$'
while IFS=$'\t' read -r evidence class rationale rest; do
  [[ -z "$evidence" || "$evidence" == \#* ]] && continue
  if [[ ! "$class" =~ $ALLOWED || -z "$rationale" || -n "${rest:-}" ]]; then
    echo "Invalid closed residue classification: $evidence" >&2
    exit 1
  fi
  path=${evidence%%:*}
  if [[ "$path" == docs/development/adr/* && "$class" == historical_documentation ]]; then
    echo "Accepted ADR residue is active, not historical: $evidence" >&2
    exit 1
  fi
  if [[ "$path" == src-tauri/src/* && "$class" == historical_documentation ]]; then
    echo "Production residue cannot be auto-classified as historical: $evidence" >&2
    exit 1
  fi
done < "$MANIFEST"
