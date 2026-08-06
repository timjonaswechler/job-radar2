/**
 * Throwaway HITL prototype for issue #310.
 * Three interaction variants live at /labs/guided-source-repair?variant=A|B|C.
 * The mock browser and Source state are intentionally in-memory and do not call production behavior.
 */
import { useEffect, useRef, useState } from "react";
import type { CSSProperties, PointerEvent as ReactPointerEvent } from "react";
import {
  AlertTriangleIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  CheckCircle2Icon,
  ChevronDownIcon,
  CircleDotIcon,
  ClipboardCheckIcon,
  CopyIcon,
  ExternalLinkIcon,
  FileWarningIcon,
  Globe2Icon,
  GripVerticalIcon,
  InfoIcon,
  KeyboardIcon,
  LockKeyholeIcon,
  Maximize2Icon,
  Minimize2Icon,
  MousePointer2Icon,
  PanelLeftIcon,
  RefreshCwIcon,
  RotateCwIcon,
  ShieldCheckIcon,
  TriangleAlertIcon,
  Undo2Icon,
  XCircleIcon,
  XIcon,
} from "lucide-react";

import "./guided-source-repair-prototype.css";

type VariantKey = "A" | "B" | "C";

type PickerOutcome = "idle" | "selected" | "not-present" | "cannot-determine";
type CheckStatus = "idle" | "running" | "passed" | "failed";

type PickerTarget = {
  id: string;
  label: string;
  value: string;
  selector: string;
  matches: number;
  attribute: string;
  previewValues: string[];
};

const variants: ReadonlyArray<{ key: VariantKey; name: string; hint: string }> = [
  { key: "A", name: "Guided split", hint: "Source context stays beside the live repair" },
  { key: "B", name: "Evidence desk", hint: "Browser first, review controls in a focused inspector" },
  { key: "C", name: "Repair case file", hint: "An ordered trail from diagnostic to application" },
];

const initialUrl = "https://jobs.acme.test/careers";

const pickerTargets: readonly PickerTarget[] = [
  {
    id: "search-heading",
    label: "Search result heading",
    value: "Open roles at ACME",
    selector: "main > section.hero > h1",
    matches: 1,
    attribute: "aria-label=\"open roles\"",
    previewValues: ["Open roles at ACME"],
  },
  {
    id: "job-title",
    label: "Posting title",
    value: "Senior Product Designer",
    selector: "article.job-card h2.job-title",
    matches: 3,
    attribute: "data-testid=\"job-title\"",
    previewValues: [
      "Senior Product Designer",
      "Product Designer, Growth",
      "Staff Product Designer",
    ],
  },
  {
    id: "location",
    label: "Posting location",
    value: "Berlin · Hybrid",
    selector: "article.job-card span.location",
    matches: 1,
    attribute: "data-field=\"location\"",
    previewValues: ["Berlin · Hybrid"],
  },
];

type DemoState = {
  sourceMode: "builtin" | "draft";
  url: string;
  history: string[];
  historyIndex: number;
  pageGeneration: number;
  previousOrigin: string;
  originChanged: boolean;
  splitWidth: number;
  pickerActive: boolean;
  hoveredTargetId: string | null;
  selectedTargetId: string | null;
  pickerOutcome: PickerOutcome;
  evidenceGeneration: number | null;
  draftRevision: number;
  reopened: boolean;
  pickerCancelledReason: string | null;
  blockedNavigation: string | null;
  announcement: string;
  check: CheckStatus;
  reviewOpen: boolean;
  applied: boolean;
  narrow: boolean;
  failureSimulation: boolean;
};

function createInitialState(): DemoState {
  return {
    sourceMode: "builtin",
    url: initialUrl,
    history: [initialUrl],
    historyIndex: 0,
    pageGeneration: 1,
    previousOrigin: originOf(initialUrl),
    originChanged: false,
    splitWidth: 340,
    pickerActive: false,
    hoveredTargetId: null,
    selectedTargetId: null,
    pickerOutcome: "idle",
    evidenceGeneration: null,
    draftRevision: 1,
    reopened: false,
    pickerCancelledReason: null,
    blockedNavigation: null,
    announcement: "Copy the Built-in Source to a draft before collecting evidence.",
    check: "idle",
    reviewOpen: false,
    applied: false,
    narrow: false,
    failureSimulation: false,
  };
}

function originOf(url: string) {
  try {
    return new URL(url).origin;
  } catch {
    return "unknown";
  }
}

function originLabel(url: string) {
  try {
    return new URL(url).host;
  } catch {
    return "unknown origin";
  }
}

function targetById(id: string | null) {
  return pickerTargets.find((target) => target.id === id) ?? null;
}

function useRepairPrototypeState() {
  const [state, setState] = useState<DemoState>(createInitialState);
  const stateRef = useRef(state);

  useEffect(() => {
    stateRef.current = state;
  }, [state]);

  const copyToDraft = () => {
    setState((current) => ({
      ...current,
      sourceMode: "draft",
      pickerActive: true,
      pickerCancelledReason: null,
      reopened: false,
      draftRevision: current.draftRevision + 1,
      announcement:
        "Copied to a new custom Source draft. The Built-in Source remains unchanged.",
    }));
  };

  const beginPicker = () => {
    setState((current) => {
      if (current.sourceMode === "builtin") {
        return {
          ...current,
          announcement:
            "This Built-in Source is read-only. Copy it to a draft before repair.",
        };
      }

      return {
        ...current,
        pickerActive: true,
        hoveredTargetId: null,
        pickerCancelledReason: null,
        announcement:
          "Picker active. Hover for a deterministic preview, then select one page element.",
      };
    });
  };

  const cancelPicker = (reason = "Picker cancelled by the user") => {
    setState((current) => ({
      ...current,
      pickerActive: false,
      hoveredTargetId: null,
      pickerCancelledReason: reason,
      announcement: `${reason}. The resumable draft was kept.`,
    }));
  };

  const selectTarget = (id: string) => {
    setState((current) => {
      if (!current.pickerActive) {
        return {
          ...current,
          announcement: "Start the picker before selecting a page element.",
        };
      }

      const target = targetById(id);
      if (!target) return current;

      return {
        ...current,
        pickerActive: false,
        hoveredTargetId: null,
        selectedTargetId: target.id,
        pickerOutcome: "selected",
        evidenceGeneration: current.pageGeneration,
        pickerCancelledReason: null,
        reopened: false,
        check: "idle",
        draftRevision: current.draftRevision + 1,
        announcement: `Selected ${target.label}. The extracted preview is advisory until the full Source Live Check.`,
      };
    });
  };

  const answerNotPresent = () => {
    setState((current) => ({
      ...current,
      pickerActive: false,
      hoveredTargetId: null,
      selectedTargetId: null,
      pickerOutcome: "not-present",
      evidenceGeneration: current.pageGeneration,
      check: "idle",
      draftRevision: current.draftRevision + 1,
      announcement:
        "Recorded ‘not present’. Posting title is required, so this step still blocks application.",
    }));
  };

  const answerCannotDetermine = () => {
    setState((current) => ({
      ...current,
      pickerActive: false,
      hoveredTargetId: null,
      selectedTargetId: null,
      pickerOutcome: "cannot-determine",
      evidenceGeneration: current.pageGeneration,
      check: "idle",
      draftRevision: current.draftRevision + 1,
      announcement:
        "Recorded ‘cannot determine’. The unresolved step remains visible and blocks application.",
    }));
  };

  const navigate = (rawUrl: string) => {
    let parsed: URL;
    try {
      parsed = new URL(rawUrl.trim());
    } catch {
      setState((current) => ({
        ...current,
        blockedNavigation: "Enter a complete HTTP(S) URL.",
        announcement: "Navigation blocked. Only complete HTTP(S) URLs are supported.",
      }));
      return;
    }

    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      setState((current) => ({
        ...current,
        blockedNavigation: `Blocked scheme: ${parsed.protocol.replace(":", "") || "unknown"}.`,
        announcement:
          "Navigation blocked. Interactive Source Session only allows HTTP(S) destinations.",
      }));
      return;
    }

    const nextUrl = parsed.toString();
    setState((current) => {
      const nextHistory = [
        ...current.history.slice(0, current.historyIndex + 1),
        nextUrl,
      ];
      const nextOrigin = originOf(nextUrl);

      return {
        ...current,
        url: nextUrl,
        history: nextHistory,
        historyIndex: nextHistory.length - 1,
        pageGeneration: current.pageGeneration + 1,
        previousOrigin: originOf(current.url),
        originChanged: nextOrigin !== originOf(current.url),
        pickerActive: false,
        hoveredTargetId: null,
        pickerCancelledReason: current.pickerActive
          ? "Picker cancelled because the page navigated"
          : null,
        blockedNavigation: null,
        announcement: `Loaded ${originLabel(nextUrl)}. Page evidence is stale until revalidated.`,
      };
    });
  };

  const moveHistory = (direction: -1 | 1) => {
    setState((current) => {
      const nextIndex = current.historyIndex + direction;
      if (nextIndex < 0 || nextIndex >= current.history.length) return current;

      const nextUrl = current.history[nextIndex];
      return {
        ...current,
        url: nextUrl,
        historyIndex: nextIndex,
        pageGeneration: current.pageGeneration + 1,
        previousOrigin: originOf(current.url),
        originChanged: originOf(nextUrl) !== originOf(current.url),
        pickerActive: false,
        hoveredTargetId: null,
        pickerCancelledReason: current.pickerActive
          ? "Picker cancelled because history navigation started"
          : null,
        blockedNavigation: null,
        announcement: `History navigation loaded ${originLabel(nextUrl)}.`,
      };
    });
  };

  const reload = () => {
    setState((current) => ({
      ...current,
      pageGeneration: current.pageGeneration + 1,
      originChanged: false,
      pickerActive: false,
      hoveredTargetId: null,
      pickerCancelledReason: current.pickerActive
        ? "Picker cancelled because the page reloaded"
        : null,
      blockedNavigation: null,
      announcement: "Page reloaded. Existing evidence is stale until revalidated.",
    }));
  };

  const revalidate = () => {
    setState((current) => {
      if (!current.selectedTargetId) {
        return {
          ...current,
          announcement: "There is no selected Element Evidence to revalidate.",
        };
      }

      return {
        ...current,
        evidenceGeneration: current.pageGeneration,
        reopened: false,
        check: "idle",
        announcement:
          "Element Evidence revalidated on the current page. A complete Source Live Check is still required.",
      };
    });
  };

  const reopenDraft = () => {
    setState((current) => ({
      ...current,
      reopened: true,
      evidenceGeneration: current.selectedTargetId ? null : current.evidenceGeneration,
      check: "idle",
      reviewOpen: false,
      announcement:
        "Resumed the saved Repair Draft. Browser state was not restored; current-page evidence needs revalidation.",
    }));
  };

  const runCheck = () => {
    const current = stateRef.current;
    const selected = Boolean(current.selectedTargetId);
    const fresh = selected && current.evidenceGeneration === current.pageGeneration;
    if (current.sourceMode === "builtin" || !selected || !fresh) {
      setState((next) => ({
        ...next,
        announcement:
          "Resolve the current Discovery step and revalidate its evidence before running the full check.",
      }));
      return;
    }

    setState((next) => ({
      ...next,
      check: "running",
      reviewOpen: false,
      announcement: "Running compiler validation and the complete Source Live Check…",
    }));

    window.setTimeout(() => {
      setState((next) => ({
        ...next,
        check: next.failureSimulation ? "failed" : "passed",
        announcement: next.failureSimulation
          ? "Source Live Check failed on the candidate selector. Return to the Discovery step."
          : "Complete Source Live Check finished. Review the candidate before applying it.",
      }));
    }, 650);
  };

  const openReview = () => {
    setState((current) => ({
      ...current,
      reviewOpen: true,
      announcement: "Final review opened. Nothing is applied until explicit confirmation.",
    }));
  };

  const apply = () => {
    setState((current) => {
      const fresh =
        current.selectedTargetId !== null &&
        current.evidenceGeneration === current.pageGeneration;
      if (
        current.sourceMode === "builtin" ||
        current.check !== "passed" ||
        !fresh
      ) {
        return {
          ...current,
          announcement:
            "Application is blocked until the custom draft has a fresh, passing complete check.",
        };
      }

      return {
        ...current,
        applied: true,
        reviewOpen: false,
        announcement:
          "Applied atomically to the custom Source draft. The Built-in Source was not changed.",
      };
    });
  };

  const reset = () => setState(createInitialState());

  return {
    state,
    actions: {
      copyToDraft,
      beginPicker,
      cancelPicker,
      selectTarget,
      answerNotPresent,
      answerCannotDetermine,
      navigate,
      moveHistory,
      reload,
      revalidate,
      reopenDraft,
      runCheck,
      openReview,
      apply,
      reset,
      setHoveredTarget: (id: string | null) =>
        setState((current) =>
          current.pickerActive ? { ...current, hoveredTargetId: id } : current,
        ),
      setSplitWidth: (width: number) =>
        setState((current) => ({
          ...current,
          splitWidth: Math.min(520, Math.max(260, width)),
        })),
      toggleNarrow: () =>
        setState((current) => ({ ...current, narrow: !current.narrow })),
      toggleFailureSimulation: () =>
        setState((current) => ({
          ...current,
          failureSimulation: !current.failureSimulation,
          check: "idle",
          announcement: current.failureSimulation
            ? "Live-check mismatch simulation disabled."
            : "Live-check mismatch simulation enabled for the error state.",
        })),
      closeReview: () => setState((current) => ({ ...current, reviewOpen: false })),
    },
  };
}

type PrototypeActions = ReturnType<typeof useRepairPrototypeState>["actions"];

type SurfaceProps = {
  state: DemoState;
  actions: PrototypeActions;
  urlDraft: string;
  setUrlDraft: (value: string) => void;
};

function PrototypeTopbar({
  variant,
  state,
  actions,
}: {
  variant: VariantKey;
  state: DemoState;
  actions: PrototypeActions;
}) {
  const variantInfo = variants.find((item) => item.key === variant) ?? variants[0];

  return (
    <header className="gsp-topbar">
      <div className="gsp-brand-lockup">
        <div className="gsp-brand-mark" aria-hidden="true">
          <MousePointer2Icon />
        </div>
        <div>
          <div className="gsp-eyebrow">Issue #310 · HITL interaction prototype</div>
          <h1>Guided Source Repair</h1>
        </div>
      </div>
      <div className="gsp-topbar-center">
        <span className="gsp-prototype-badge">PROTOTYPE · no production behavior</span>
        <span className="gsp-variant-caption">
          {variant} · {variantInfo.name}
        </span>
      </div>
      <div className="gsp-topbar-actions">
        <span className={`gsp-status-pill ${state.sourceMode === "draft" ? "is-draft" : "is-builtin"}`}>
          {state.sourceMode === "draft" ? "Custom draft" : "Built-in · read-only"}
        </span>
        <button
          type="button"
          className="gsp-quiet-button"
          onClick={actions.toggleFailureSimulation}
          aria-pressed={state.failureSimulation}
          title="Toggle a simulated Source Live Check mismatch"
        >
          <TriangleAlertIcon />
          {state.failureSimulation ? "Mismatch on" : "Test error"}
        </button>
        <button
          type="button"
          className="gsp-quiet-button"
          onClick={actions.toggleNarrow}
          aria-pressed={state.narrow}
          title="Preview the narrow-window layout"
        >
          {state.narrow ? <Maximize2Icon /> : <Minimize2Icon />}
          {state.narrow ? "Widen" : "Narrow"}
        </button>
        <button type="button" className="gsp-icon-button" onClick={actions.reset} aria-label="Reset prototype">
          <Undo2Icon />
        </button>
      </div>
    </header>
  );
}

function SourceContext({
  state,
  actions,
  compact = false,
}: {
  state: DemoState;
  actions: PrototypeActions;
  compact?: boolean;
}) {
  const isDraft = state.sourceMode === "draft";
  const selected = targetById(state.selectedTargetId);
  const evidenceStale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;

  return (
    <aside className={`gsp-source-context ${compact ? "is-compact" : ""}`}>
      <div className="gsp-section-kicker">
        <PanelLeftIcon /> Source context
        <span className="gsp-kicker-line" />
        <span>1 / 1</span>
      </div>
      <div className="gsp-source-title-row">
        <div>
          <div className="gsp-source-name">{isDraft ? "ACME Careers · repair draft" : "ACME Careers"}</div>
          <div className="gsp-muted-copy">{isDraft ? "custom/acme-careers-draft" : "builtin/acme-careers"}</div>
        </div>
        <span className={`gsp-mini-status ${isDraft ? "is-draft" : "is-locked"}`}>
          {isDraft ? "DRAFT" : "BUILT-IN"}
        </span>
      </div>
      {!isDraft ? (
        <div className="gsp-copy-callout">
          <div className="gsp-callout-icon"><ShieldCheckIcon /></div>
          <div>
            <strong>Repair starts from a copy</strong>
            <p>This Source is bundled and read-only. Copy its authored Source document to a custom draft; profile behavior is never mutated.</p>
            <button type="button" className="gsp-primary-button gsp-small-button" onClick={actions.copyToDraft}>
              <CopyIcon /> Copy to draft
            </button>
          </div>
        </div>
      ) : (
        <div className="gsp-draft-callout">
          <div className="gsp-callout-icon"><CheckCircle2Icon /></div>
          <div>
            <strong>Resumable draft · revision {state.draftRevision}</strong>
            <p>Authored intent is saved. Browser state, HTML, screenshots, cookies, and credentials are not.</p>
          </div>
        </div>
      )}
      <div className="gsp-field-list">
        <div className="gsp-field-row">
          <span>Source Config</span>
          <strong>host: jobs.acme.test</strong>
        </div>
        <div className="gsp-field-row">
          <span>Source Profile</span>
          <strong>career-site · v3</strong>
        </div>
        <div className="gsp-field-row">
          <span>Access Path</span>
          <strong>browser · rendered HTML</strong>
        </div>
      </div>
      <div className="gsp-repair-scope">
        <div className="gsp-scope-header">
          <span className="gsp-scope-label">Repair scope</span>
          <span className="gsp-focus-chip">DISCOVERY ONLY</span>
        </div>
        <div className="gsp-phase-row is-active">
          <CircleDotIcon />
          <div>
            <strong>Discovery</strong>
            <span>Posting title · required</span>
          </div>
          <span className={`gsp-step-state ${selected && !evidenceStale ? "is-good" : "is-open"}`}>
            {selected && !evidenceStale ? "ready" : evidenceStale ? "stale" : "open"}
          </span>
        </div>
        <div className="gsp-phase-row is-muted">
          <CheckCircle2Icon />
          <div>
            <strong>Detail</strong>
            <span>descriptionText · unchanged</span>
          </div>
          <span className="gsp-step-state is-good">checked</span>
        </div>
        <p className="gsp-scope-note">Only the failing/incomplete phase is open. Detail is visible for context but is not being edited.</p>
      </div>
      <div className="gsp-diagnostic-list">
        <div className="gsp-diagnostic is-error">
          <FileWarningIcon />
          <div><strong>DISCOVERY · required field</strong><span>Posting title selector is missing</span></div>
        </div>
        <div className="gsp-diagnostic is-warning">
          <AlertTriangleIcon />
          <div><strong>DETAIL · warning</strong><span>Evidence was last checked on an older generation</span></div>
        </div>
      </div>
      <div className="gsp-context-footer">
        <span>Source generation <strong>42</strong></span>
        <button type="button" className="gsp-link-button" onClick={actions.reopenDraft}>
          Reopen saved draft
        </button>
      </div>
      {state.reopened ? <div className="gsp-inline-warning"><AlertTriangleIcon /> Reopened without browser state · revalidation required</div> : null}
    </aside>
  );
}

function SplitHandle({
  width,
  onChange,
}: {
  width: number;
  onChange: (width: number) => void;
}) {
  const startX = useRef(0);
  const startWidth = useRef(width);
  const [dragging, setDragging] = useState(false);

  const handlePointerDown = (event: ReactPointerEvent<HTMLButtonElement>) => {
    startX.current = event.clientX;
    startWidth.current = width;
    setDragging(true);
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const handlePointerMove = (event: ReactPointerEvent<HTMLButtonElement>) => {
    if (!dragging) return;
    onChange(startWidth.current + event.clientX - startX.current);
  };

  const handlePointerUp = (event: ReactPointerEvent<HTMLButtonElement>) => {
    setDragging(false);
    event.currentTarget.releasePointerCapture(event.pointerId);
  };

  return (
    <button
      type="button"
      className={`gsp-split-handle ${dragging ? "is-dragging" : ""}`}
      onPointerDown={handlePointerDown}
      onPointerMove={handlePointerMove}
      onPointerUp={handlePointerUp}
      aria-label="Resize Source context and browser split"
      title="Drag to resize the split"
    >
      <GripVerticalIcon />
    </button>
  );
}

function BrowserSurface({ state, actions, urlDraft, setUrlDraft }: SurfaceProps & { compact?: boolean }) {
  const selected = targetById(state.selectedTargetId);
  const hovered = targetById(state.hoveredTargetId);
  const previewTarget = hovered ?? selected;
  const evidenceStale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;
  const currentOrigin = originLabel(state.url);
  const external = state.url.includes("partner.example.test");
  const urlInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!state.pickerActive) return;
    urlInputRef.current?.blur();
  }, [state.pickerActive]);

  const submitUrl = () => actions.navigate(urlDraft);

  return (
    <section className="gsp-browser-surface" aria-label="Interactive Source Session browser area">
      <div className="gsp-browser-toolbar">
        <div className="gsp-browser-nav-buttons">
          <button type="button" className="gsp-browser-button" onClick={() => actions.moveHistory(-1)} disabled={state.historyIndex === 0} aria-label="Back" title="Back">
            <ArrowLeftIcon />
          </button>
          <button type="button" className="gsp-browser-button" onClick={() => actions.moveHistory(1)} disabled={state.historyIndex === state.history.length - 1} aria-label="Forward" title="Forward">
            <ArrowRightIcon />
          </button>
        </div>
        <div className="gsp-address-bar">
          <LockKeyholeIcon className="gsp-address-lock" aria-hidden="true" />
          <input
            ref={urlInputRef}
            value={urlDraft}
            onChange={(event) => setUrlDraft(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") submitUrl();
            }}
            aria-label="Editable browser URL"
            spellCheck={false}
          />
          <button type="button" className="gsp-go-button" onClick={submitUrl}>Go</button>
        </div>
        <span className={`gsp-origin-chip ${state.originChanged ? "is-changed" : ""}`}>
          <Globe2Icon /> {currentOrigin}
          {state.originChanged ? <span className="gsp-origin-change-dot" title="Origin changed" /> : null}
        </span>
        <button type="button" className="gsp-browser-button" onClick={actions.reload} aria-label="Reload" title="Reload">
          <RotateCwIcon />
        </button>
      </div>
      <div className="gsp-browser-meta-row">
        <span><span className="gsp-live-dot" /> Interactive Source Session · one page</span>
        <span>Page generation <strong>{state.pageGeneration}</strong></span>
      </div>
      {state.blockedNavigation ? (
        <div className="gsp-browser-alert" role="alert">
          <XCircleIcon />
          <div><strong>Navigation blocked</strong><span>{state.blockedNavigation} Current page and draft are unchanged.</span></div>
          <button type="button" className="gsp-icon-button" onClick={() => actions.navigate(state.url)} aria-label="Dismiss navigation error"><XIcon /></button>
        </div>
      ) : null}
      <div
        className={`gsp-browser-viewport ${state.pickerActive ? "is-picking" : ""}`}
        tabIndex={0}
        aria-label="Mock career page. Tab to page elements; Escape cancels the picker."
      >
        <div className="gsp-mock-site">
          <div className="gsp-mock-nav">
            <div className="gsp-mock-logo"><span>AC</span> ACME Careers</div>
            <div className="gsp-mock-nav-links"><span>Teams</span><span>Life at ACME</span><span>About</span></div>
            <span className="gsp-mock-site-host">{currentOrigin}</span>
          </div>
          {state.originChanged ? (
            <div className="gsp-origin-banner"><Globe2Icon /><span>Origin changed</span><strong>{originLabel(state.previousOrigin)} → {currentOrigin}</strong><small>Recheck evidence on this current document.</small></div>
          ) : null}
          <div className="gsp-mock-content">
            <div className="gsp-mock-breadcrumb">ACME Careers <span>/</span> {external ? "Partner board" : "Open roles"}</div>
            <div className="gsp-mock-hero">
              <div>
                <span className="gsp-mock-overline">{external ? "Partner mirror" : "Build what matters"}</span>
                <button
                  type="button"
                  className={`gsp-page-target gsp-heading-target ${state.hoveredTargetId === "search-heading" ? "is-hovered" : ""} ${state.selectedTargetId === "search-heading" ? "is-selected" : ""}`}
                  onMouseEnter={() => actions.setHoveredTarget("search-heading")}
                  onMouseLeave={() => actions.setHoveredTarget(null)}
                  onFocus={() => actions.setHoveredTarget("search-heading")}
                  onBlur={() => actions.setHoveredTarget(null)}
                  onClick={() => actions.selectTarget("search-heading")}
                >
                  {external ? "Roles available through partner board" : "Open roles at ACME"}
                </button>
                <p>{external ? "This is a different origin with a similar visual shape." : "Find the role where your curiosity and craft can make an impact."}</p>
              </div>
              <div className="gsp-mock-hero-orb"><span>03</span><small>open teams</small></div>
            </div>
            <div className="gsp-mock-results-header"><span>Showing 12 opportunities</span><button type="button">Filter <ChevronDownIcon /></button></div>
            <div className="gsp-job-grid">
              {(external ? ["Operations Lead", "Research Engineer"] : ["Senior Product Designer", "Product Designer, Growth", "Staff Product Designer"]).map((title, index) => (
                <article className="gsp-mock-job-card" key={title}>
                  <div className="gsp-job-card-top"><span className="gsp-job-index">0{index + 1}</span><ExternalLinkIcon /></div>
                  <button
                    type="button"
                    className={`gsp-page-target gsp-job-title-target ${state.hoveredTargetId === "job-title" ? "is-hovered" : ""} ${state.selectedTargetId === "job-title" ? "is-selected" : ""}`}
                    onMouseEnter={() => actions.setHoveredTarget("job-title")}
                    onMouseLeave={() => actions.setHoveredTarget(null)}
                    onFocus={() => actions.setHoveredTarget("job-title")}
                    onBlur={() => actions.setHoveredTarget(null)}
                    onClick={() => actions.selectTarget("job-title")}
                  >
                    {title}
                  </button>
                  <button
                    type="button"
                    className={`gsp-page-target gsp-location-target ${state.hoveredTargetId === "location" ? "is-hovered" : ""} ${state.selectedTargetId === "location" ? "is-selected" : ""}`}
                    onMouseEnter={() => actions.setHoveredTarget("location")}
                    onMouseLeave={() => actions.setHoveredTarget(null)}
                    onFocus={() => actions.setHoveredTarget("location")}
                    onBlur={() => actions.setHoveredTarget(null)}
                    onClick={() => actions.selectTarget("location")}
                  >
                    {external ? "Remote · Europe" : "Berlin · Hybrid"}
                  </button>
                  <div className="gsp-job-card-footer"><span>{external ? "Partner board" : "Design"}</span><span>Full-time</span></div>
                </article>
              ))}
            </div>
          </div>
          <div className="gsp-mock-footer">Mock page for interaction review · no page content leaves this prototype</div>
        </div>
        <div className="gsp-picker-instruction" role="status" aria-live="polite">
          <div className="gsp-instruction-top"><span className="gsp-picking-dot" /> {state.pickerActive ? "Picker active" : "Repair step"}<span className="gsp-instruction-generation">generation {state.pageGeneration}</span></div>
          <strong>Find the element for <em>Posting title</em></strong>
          <p>{state.pickerActive ? "Hover for a deterministic preview. Click to collect bounded Element Evidence." : selected ? "Evidence captured. Re-pick if the current page or preview is not trustworthy." : "Copy to draft, then start the picker to answer this one question."}</p>
          {state.pickerActive ? (
            <div className="gsp-instruction-actions"><kbd>Esc</kbd><span>cancel picker</span><button type="button" className="gsp-link-button" onClick={() => actions.cancelPicker()}>Cancel</button></div>
          ) : null}
        </div>
        {previewTarget ? (
          <div className={`gsp-element-preview ${evidenceStale ? "is-stale" : ""}`}>
            <div className="gsp-preview-header"><span><MousePointer2Icon /> {state.pickerActive ? "Live preview" : "Captured evidence"}</span><span className={`gsp-match-pill ${previewTarget.matches > 1 ? "is-many" : "is-one"}`}>{previewTarget.matches} match{previewTarget.matches === 1 ? "" : "es"}</span></div>
            <strong>{previewTarget.label}</strong>
            <code>{previewTarget.selector}</code>
            <div className="gsp-preview-values">{previewTarget.previewValues.map((value, index) => <span key={value}><b>{index + 1}</b>{value}</span>)}</div>
            <div className="gsp-preview-footer"><span>{previewTarget.attribute}</span>{evidenceStale ? <span className="gsp-stale-text"><AlertTriangleIcon /> stale after navigation</span> : <span className="gsp-fresh-text"><CheckCircle2Icon /> current page</span>}</div>
          </div>
        ) : null}
      </div>
      <div className="gsp-browser-footer">
        <span><KeyboardIcon /> Tab focuses controls · Esc cancels active picker</span>
        {state.pickerCancelledReason ? <span className="gsp-cancelled-note"><XCircleIcon /> {state.pickerCancelledReason}</span> : <span>Downloads and unsupported schemes are blocked</span>}
      </div>
    </section>
  );
}

function EvidenceCard({ state, actions }: { state: DemoState; actions: PrototypeActions }) {
  const selected = targetById(state.selectedTargetId);
  const stale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;
  return (
    <div className="gsp-evidence-card">
      <div className="gsp-card-heading"><div><span className="gsp-card-kicker">Element Evidence</span><h2>{selected ? selected.label : "No answer yet"}</h2></div>{selected ? <span className={`gsp-evidence-status ${stale ? "is-stale" : "is-fresh"}`}>{stale ? "STALE" : "FRESH"}</span> : <span className="gsp-evidence-status is-open">OPEN</span>}</div>
      {selected ? (
        <>
          <div className="gsp-evidence-selector"><code>{selected.selector}</code><span>{selected.matches} matches</span></div>
          <div className="gsp-evidence-extract"><span>Deterministic extracted-value preview</span><strong>{selected.value}</strong>{selected.matches > 1 ? <p><AlertTriangleIcon /> Multiple matches are visible. The final managed-Chrome check decides whether this selector is acceptable.</p> : null}</div>
          {stale ? <div className="gsp-stale-callout"><AlertTriangleIcon /><span>Navigation or reopen invalidated this page evidence.</span><button type="button" className="gsp-secondary-button gsp-small-button" onClick={actions.revalidate}>Revalidate</button></div> : null}
        </>
      ) : (
        <div className="gsp-empty-evidence"><MousePointer2Icon /><span>Use the browser picker to collect a bounded selector, match count, safe attributes, and normalized preview.</span></div>
      )}
    </div>
  );
}

function RepairActions({ state, actions }: { state: DemoState; actions: PrototypeActions }) {
  const selected = targetById(state.selectedTargetId);
  const stale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;
  const canCheck = state.sourceMode === "draft" && Boolean(selected) && !stale && state.pickerOutcome === "selected";
  return (
    <section className="gsp-repair-actions" aria-label="Repair step actions">
      <div className="gsp-actions-heading"><div><span className="gsp-card-kicker">Current unresolved field</span><h2>Posting title</h2></div><span className="gsp-required-pill">required</span></div>
      <p className="gsp-action-explanation">Answer this one Discovery question. Retry only replaces unconfirmed evidence; it does not add Source Behavior Language retry semantics.</p>
      <div className="gsp-action-buttons">
        <button type="button" className="gsp-primary-button" onClick={actions.beginPicker} disabled={state.sourceMode === "builtin"}>
          <MousePointer2Icon /> {state.pickerActive ? "Picker active" : selected ? "Retry selection" : "Pick an element"}
        </button>
        {state.pickerActive ? <button type="button" className="gsp-secondary-button" onClick={() => actions.cancelPicker("Picker cancelled explicitly")}><XIcon /> Cancel</button> : null}
        <button type="button" className="gsp-secondary-button" onClick={actions.answerNotPresent} disabled={state.sourceMode === "builtin"}><XCircleIcon /> Not present</button>
        <button type="button" className="gsp-secondary-button" onClick={actions.answerCannotDetermine} disabled={state.sourceMode === "builtin"}><FileWarningIcon /> Cannot determine</button>
      </div>
      {state.pickerOutcome === "not-present" ? <div className="gsp-answer-note is-warning"><AlertTriangleIcon /> Not present is recorded, but a required Posting title still blocks staging.</div> : null}
      {state.pickerOutcome === "cannot-determine" ? <div className="gsp-answer-note is-error"><XCircleIcon /> Cannot determine remains unresolved. Choose Retry to try again.</div> : null}
      <div className="gsp-check-actions">
        <button type="button" className="gsp-check-button" onClick={actions.runCheck} disabled={!canCheck || state.check === "running"}>
          {state.check === "running" ? <RefreshCwIcon className="gsp-spin" /> : <ClipboardCheckIcon />}
          {state.check === "running" ? "Checking…" : "Run full Source check"}
        </button>
        {state.check !== "idle" && state.check !== "running" ? <button type="button" className="gsp-link-button" onClick={actions.openReview}>Open final review</button> : null}
      </div>
      <div className="gsp-action-status" aria-live="polite"><InfoIcon /> {state.announcement}</div>
    </section>
  );
}

function VariantA({ props }: { props: SurfaceProps }) {
  const { state, actions } = props;
  const style = { "--gsp-context-width": `${state.splitWidth}px` } as CSSProperties;
  return (
    <div className="gsp-variant gsp-variant-a" style={style}>
      <div className="gsp-variant-label"><span>A</span><div><strong>Guided split</strong><small>Keep Source context and one browser area visible together.</small></div><span className="gsp-resize-hint"><GripVerticalIcon /> drag the divider</span></div>
      <div className="gsp-a-workspace">
        <div className="gsp-a-context"><SourceContext state={state} actions={actions} /></div>
        <SplitHandle width={state.splitWidth} onChange={actions.setSplitWidth} />
        <div className="gsp-a-browser-column">
          <BrowserSurface {...props} />
          <div className="gsp-a-bottom-grid"><EvidenceCard state={state} actions={actions} /><RepairActions state={state} actions={actions} /></div>
        </div>
      </div>
    </div>
  );
}

function VariantB({ props }: { props: SurfaceProps }) {
  const { state, actions } = props;
  return (
    <div className="gsp-variant gsp-variant-b">
      <div className="gsp-variant-label"><span>B</span><div><strong>Evidence desk</strong><small>Make the page the primary workspace; keep decisions in a focused inspector.</small></div><span className="gsp-variant-mode"><Globe2Icon /> browser-first</span></div>
      <div className="gsp-b-progress"><div className="gsp-b-progress-title"><span className="gsp-card-kicker">Repair sequence</span><strong>Discovery <span>/</span> Posting title</strong></div><div className="gsp-b-progress-track"><span className="is-done" /><span className={state.selectedTargetId && state.evidenceGeneration === state.pageGeneration ? "is-done" : "is-current"} /><span /></div><div className="gsp-b-progress-copy">1 unresolved question <span>·</span> Detail stays untouched</div></div>
      <div className="gsp-b-workspace">
        <div className="gsp-b-browser"><BrowserSurface {...props} /></div>
        <aside className="gsp-b-inspector">
          <details className="gsp-source-details" open>
            <summary><span><PanelLeftIcon /> Source snapshot</span><ChevronDownIcon /></summary>
            <SourceContext state={state} actions={actions} compact />
          </details>
          <EvidenceCard state={state} actions={actions} />
          <RepairActions state={state} actions={actions} />
        </aside>
      </div>
    </div>
  );
}

function VariantC({ props }: { props: SurfaceProps }) {
  const { state, actions } = props;
  const selected = targetById(state.selectedTargetId);
  const stale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;
  return (
    <div className="gsp-variant gsp-variant-c">
      <div className="gsp-variant-label"><span>C</span><div><strong>Repair case file</strong><small>Make each protocol boundary explicit from diagnostic through application.</small></div><span className="gsp-variant-mode"><ShieldCheckIcon /> reviewable trail</span></div>
      <div className="gsp-c-workspace">
        <aside className="gsp-c-timeline">
          <div className="gsp-case-heading"><span className="gsp-card-kicker">Source repair case</span><h2>ACME Careers</h2><span className="gsp-muted-copy">{state.sourceMode === "draft" ? "custom draft" : "Built-in Source"}</span></div>
          <div className="gsp-timeline-step is-complete"><span className="gsp-timeline-dot"><CheckCircle2Icon /></span><div><strong>01 · Open diagnostic</strong><span>Required Discovery field identified</span></div></div>
          <div className="gsp-timeline-step is-active"><span className="gsp-timeline-dot"><CircleDotIcon /></span><div><strong>02 · Collect evidence</strong><span>Posting title · one bounded answer</span></div></div>
          <div className={`gsp-timeline-step ${state.check === "passed" ? "is-complete" : ""}`}><span className="gsp-timeline-dot">{state.check === "passed" ? <CheckCircle2Icon /> : <span>03</span>}</span><div><strong>03 · Validate candidate</strong><span>Compiler + complete Source Live Check</span></div></div>
          <div className="gsp-timeline-step"><span className="gsp-timeline-dot"><span>04</span></span><div><strong>04 · Review and apply</strong><span>Explicit atomic Source replacement</span></div></div>
          <div className="gsp-case-note"><InfoIcon /><span>Built-in behavior is copied into the draft; no reusable Source Profile is changed.</span></div>
          <button type="button" className="gsp-link-button gsp-timeline-reopen" onClick={actions.reopenDraft}><RefreshCwIcon /> Reopen draft state</button>
        </aside>
        <main className="gsp-c-main">
          <BrowserSurface {...props} />
          <div className="gsp-c-ledger">
            <div className="gsp-ledger-header"><div><span className="gsp-card-kicker">Evidence ledger</span><h2>{selected ? selected.label : "Awaiting bounded evidence"}</h2></div><span className={stale ? "gsp-ledger-state is-stale" : selected ? "gsp-ledger-state is-fresh" : "gsp-ledger-state"}>{stale ? "revalidation required" : selected ? "current page" : "not started"}</span></div>
            <div className="gsp-ledger-grid"><div><span>Draft revision</span><strong>r{state.draftRevision}</strong></div><div><span>Page generation</span><strong>g{state.pageGeneration}</strong></div><div><span>Picker terminal</span><strong>{state.pickerOutcome === "idle" ? "none" : state.pickerOutcome.replace("-", " ")}</strong></div><div><span>Check</span><strong>{state.check === "idle" ? "not run" : state.check}</strong></div></div>
            <RepairActions state={state} actions={actions} />
          </div>
        </main>
      </div>
    </div>
  );
}

function FinalReview({ state, actions }: { state: DemoState; actions: PrototypeActions }) {
  const selected = targetById(state.selectedTargetId);
  const stale = Boolean(selected) && state.evidenceGeneration !== state.pageGeneration;
  const failure = state.check === "failed";
  return (
    <div className="gsp-review-backdrop" role="presentation">
      <section className="gsp-review-dialog" role="dialog" aria-modal="true" aria-labelledby="gsp-review-title">
        <div className="gsp-review-header"><div><span className="gsp-card-kicker">Guided Source Repair · final review</span><h2 id="gsp-review-title">Review before applying this Source</h2><p>One immutable Repair Proposal · no writes have happened yet.</p></div><button type="button" className="gsp-icon-button" onClick={actions.closeReview} aria-label="Close final review"><XIcon /></button></div>
        <div className="gsp-review-body">
          <div className="gsp-review-summary"><div className="gsp-review-source"><span className="gsp-summary-icon"><ShieldCheckIcon /></span><div><strong>ACME Careers · custom draft</strong><span>Source generation 42 · draft revision {state.draftRevision}</span></div></div><span className={`gsp-review-result ${failure ? "is-failure" : state.check === "passed" ? "is-pass" : "is-open"}`}>{failure ? "CHECK FAILED" : state.check === "passed" ? "READY TO APPLY" : "INCOMPLETE"}</span></div>
          <div className="gsp-review-columns">
            <div className="gsp-review-checks"><h3>Complete check results</h3>
              <ReviewCheckRow label="Source/Profile generation" status="42 · matches draft" kind="pass" />
              <ReviewCheckRow label="Profile Compiler" status="Passed · effective plan compiled" kind="pass" />
              <ReviewCheckRow label="Discovery · browser strategy" status={failure ? "Failed · selector did not replay" : "Passed · candidate accepted"} kind={failure ? "failure" : "pass"} />
              <ReviewCheckRow label="Detail · descriptionText" status="Passed · unchanged" kind="pass" />
              <ReviewCheckRow label="Freshness binding" status={stale ? "Failed · page evidence is stale" : "Passed · current generation"} kind={stale ? "failure" : "pass"} />
              <ReviewCheckRow label="Application admission" status={failure || stale ? "Blocked" : "Explicit confirmation required"} kind={failure || stale ? "failure" : "warning"} />
            </div>
            <div className="gsp-review-diff"><h3>Authored Source difference</h3><div className="gsp-diff-code"><span className="gsp-diff-label">Direct Source Specialization · discovery</span>{selected ? <><code><em>selector</em></code><code className="gsp-diff-add">+ {selected.selector}</code><code><em>preview</em></code><code className="gsp-diff-add">+ {selected.value}</code></> : <code className="gsp-diff-muted">No candidate selector has been staged.</code>}</div><div className="gsp-review-warning-list"><div><AlertTriangleIcon /><span>Picker evidence from the system Webview is advisory; managed Chrome remains authoritative.</span></div>{selected && selected.matches > 1 ? <div><AlertTriangleIcon /><span>{selected.matches} matches were previewed. Review the final normalized result before applying.</span></div> : null}{state.originChanged ? <div><Globe2Icon /><span>Origin changed during the session. Evidence is bound to the current page generation.</span></div> : null}</div></div>
          </div>
          {failure ? <div className="gsp-review-error" role="alert"><XCircleIcon /><div><strong>Source Live Check returned a mismatch</strong><span>The candidate is retained in the Repair Draft. Return to Discovery, retry or choose a different element, then run the complete check again.</span></div></div> : null}
          {!failure && state.check === "passed" ? <div className="gsp-review-atomic-note"><ShieldCheckIcon /><span><strong>Atomic apply</strong> replaces exactly one custom Source document after confirmation. The Built-in Source and reusable Source Profile remain unchanged.</span></div> : null}
        </div>
        <div className="gsp-review-footer"><button type="button" className="gsp-secondary-button" onClick={actions.closeReview}>{failure ? "Return to repair step" : "Keep editing"}</button><button type="button" className="gsp-primary-button" onClick={actions.apply} disabled={failure || stale || state.check !== "passed"}><ShieldCheckIcon /> Apply Source atomically</button></div>
      </section>
    </div>
  );
}

function ReviewCheckRow({ label, status, kind }: { label: string; status: string; kind: "pass" | "warning" | "failure" }) {
  const Icon = kind === "pass" ? CheckCircle2Icon : kind === "warning" ? AlertTriangleIcon : XCircleIcon;
  return <div className={`gsp-review-check-row is-${kind}`}><Icon /><span>{label}</span><strong>{status}</strong></div>;
}

function PrototypeSwitcher({ variant, onChange }: { variant: VariantKey; onChange: (variant: VariantKey) => void }) {
  const currentIndex = variants.findIndex((item) => item.key === variant);
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (target?.matches("input, textarea, select, button, [contenteditable='true']")) return;
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      event.preventDefault();
      const offset = event.key === "ArrowRight" ? 1 : -1;
      const next = (currentIndex + offset + variants.length) % variants.length;
      onChange(variants[next].key);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [currentIndex, onChange]);

  if (!import.meta.env.DEV) return null;
  const previous = variants[(currentIndex - 1 + variants.length) % variants.length];
  const next = variants[(currentIndex + 1) % variants.length];
  return (
    <nav className="gsp-variant-switcher" aria-label="Prototype variants">
      <button type="button" onClick={() => onChange(previous.key)} aria-label={`Previous variant: ${previous.name}`}><ArrowLeftIcon /></button>
      <span><b>{variant}</b> — {variants[currentIndex].name}</span>
      <button type="button" onClick={() => onChange(next.key)} aria-label={`Next variant: ${next.name}`}><ArrowRightIcon /></button>
    </nav>
  );
}

function readVariant(): VariantKey {
  const value = new URLSearchParams(window.location.search).get("variant");
  return value === "B" || value === "C" ? value : "A";
}

export function GuidedSourceRepairPrototype() {
  const [variant, setVariant] = useState<VariantKey>(readVariant);
  const [urlDraft, setUrlDraft] = useState(initialUrl);
  const { state, actions } = useRepairPrototypeState();

  useEffect(() => {
    const handleLocationChange = () => setVariant(readVariant());
    window.addEventListener("popstate", handleLocationChange);
    return () => window.removeEventListener("popstate", handleLocationChange);
  }, []);

  useEffect(() => setUrlDraft(state.url), [state.url]);

  const changeVariant = (next: VariantKey) => {
    const params = new URLSearchParams(window.location.search);
    params.set("variant", next);
    window.history.replaceState(null, "", `${window.location.pathname}?${params.toString()}`);
    setVariant(next);
  };

  useEffect(() => {
    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape" && state.pickerActive) {
        actions.cancelPicker("Picker cancelled with Escape");
      }
    };
    window.addEventListener("keydown", handleEscape);
    return () => window.removeEventListener("keydown", handleEscape);
  }, [actions, state.pickerActive]);

  const surfaceProps: SurfaceProps = { state, actions, urlDraft, setUrlDraft };
  return (
    <div className={`guided-source-repair-prototype ${state.narrow ? "is-narrow" : ""}`}>
      <PrototypeTopbar variant={variant} state={state} actions={actions} />
      <div className="gsp-prototype-banner"><InfoIcon /><span><strong>Question:</strong> which split-view interaction makes Guided Source Repair understandable, correctable, and trustworthy from diagnostic entry through final Source application?</span><span className="gsp-banner-separator" /><span>Try the picker, navigation races, stale draft, error, narrow-window, and review states.</span></div>
      <main className="gsp-prototype-main">
        {variant === "A" ? <VariantA props={surfaceProps} /> : variant === "B" ? <VariantB props={surfaceProps} /> : <VariantC props={surfaceProps} />}
      </main>
      {state.applied ? <div className="gsp-applied-toast" role="status"><CheckCircle2Icon /><div><strong>Applied to custom Source draft</strong><span>Atomic apply complete · Built-in Source unchanged</span></div><button type="button" onClick={actions.reset} aria-label="Reset after apply"><Undo2Icon /></button></div> : null}
      {state.reviewOpen ? <FinalReview state={state} actions={actions} /> : null}
      <PrototypeSwitcher variant={variant} onChange={changeVariant} />
    </div>
  );
}
