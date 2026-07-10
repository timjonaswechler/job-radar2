import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
} from "react";
import { createPortal } from "react-dom";

import { CheckIcon, ChevronDownIcon } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { SourceConfigEntry } from "@/features/sources/shared/source-config-schema";

export type ConfigKeyOption = {
  key: string;
  label: string;
  required: boolean;
};

type ConfigKeyControlProps = {
  entry: SourceConfigEntry;
  index: number;
  keyOptions: ConfigKeyOption[];
  locked: boolean;
  disabled: boolean;
  portalContainer?: HTMLElement | null;
  onChange: (key: string) => void;
};

export function ConfigKeyControl({
  entry,
  index,
  keyOptions,
  locked,
  disabled,
  portalContainer,
  onChange,
}: ConfigKeyControlProps) {
  const [open, setOpen] = useState(false);
  const [popoverStyle, setPopoverStyle] = useState<CSSProperties | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const popoverRef = useRef<HTMLDivElement | null>(null);
  const inputLocked = disabled || locked;
  const popoverRoot = portalContainer ?? document.body;

  const updatePopoverPosition = useCallback(() => {
    if (!inputRef.current || !popoverRoot) return;

    const inputRect = inputRef.current.getBoundingClientRect();
    const rootRect =
      popoverRoot === document.body
        ? { top: 0, left: 0, right: window.innerWidth }
        : popoverRoot.getBoundingClientRect();
    const minWidth = 256;
    const width = Math.max(inputRect.width, minWidth);
    const left = Math.min(
      Math.max(inputRect.left - rootRect.left, 8),
      Math.max(rootRect.right - rootRect.left - width - 8, 8),
    );

    setPopoverStyle({
      position: popoverRoot === document.body ? "fixed" : "absolute",
      top: inputRect.bottom - rootRect.top + 4,
      left,
      width,
    });
  }, [popoverRoot]);

  useLayoutEffect(() => {
    if (!open) return;
    updatePopoverPosition();
  }, [open, updatePopoverPosition]);

  useEffect(() => {
    if (!open) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (containerRef.current?.contains(target)) return;
      if (popoverRef.current?.contains(target)) return;
      setOpen(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("resize", updatePopoverPosition);
    document.addEventListener("scroll", updatePopoverPosition, true);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("resize", updatePopoverPosition);
      document.removeEventListener("scroll", updatePopoverPosition, true);
    };
  }, [open, updatePopoverPosition]);

  const chooseKey = (key: string) => {
    setOpen(false);
    onChange(key);
  };

  return (
    <div ref={containerRef} className="relative" data-vaul-no-drag="">
      <Input
        ref={inputRef}
        value={entry.key}
        onChange={(event) => onChange(event.target.value)}
        onFocus={() => {
          if (!inputLocked) setOpen(true);
        }}
        onClick={() => {
          if (!inputLocked) setOpen(true);
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") setOpen(false);
        }}
        aria-label={`Key für Konfigurationswert ${index + 1}`}
        placeholder="Key"
        className="h-8 rounded-none border-0 bg-transparent pr-8 shadow-none ring-0 focus-visible:ring-0"
        disabled={inputLocked}
        data-vaul-no-drag=""
      />
      {keyOptions.length && !inputLocked ? (
        <Button
          type="button"
          variant="ghost"
          size="icon-xs"
          className="absolute top-1/2 right-1 -translate-y-1/2"
          onMouseDown={(event) => event.preventDefault()}
          onClick={() => setOpen((current) => !current)}
          aria-label="Schema-Key-Auswahl öffnen"
          aria-expanded={open}
          data-vaul-no-drag=""
        >
          <ChevronDownIcon aria-hidden="true" />
        </Button>
      ) : null}
      {open && keyOptions.length && popoverStyle
        ? createPortal(
            <div
              ref={popoverRef}
              className="z-50 overflow-hidden rounded-lg bg-popover text-popover-foreground shadow-md ring-1 ring-foreground/10"
              style={popoverStyle}
              role="listbox"
              data-vaul-no-drag=""
            >
              <div className="px-2 py-1.5 text-xs text-muted-foreground">
                Bekannte Schema-Keys
              </div>
              <div className="h-px bg-border/50" />
              <div className="max-h-72 overflow-y-auto p-1">
                {keyOptions.map((option) => {
                  const selected = option.key === entry.key;

                  return (
                    <button
                      key={option.key}
                      type="button"
                      className="relative flex min-h-7 w-full cursor-default items-center rounded-md px-2 py-1 text-left text-xs/relaxed outline-hidden hover:bg-accent hover:text-accent-foreground focus:bg-accent focus:text-accent-foreground"
                      role="option"
                      aria-selected={selected}
                      onMouseDown={(event) => event.preventDefault()}
                      onClick={() => chooseKey(option.key)}
                      data-vaul-no-drag=""
                    >
                      <div className="flex min-w-0 flex-col gap-0.5 pr-6">
                        <span className="truncate font-medium">{option.key}</span>
                        <span className="truncate text-muted-foreground">
                          {option.label}
                          {option.required ? " · Pflicht" : ""}
                        </span>
                      </div>
                      {selected ? (
                        <CheckIcon
                          className="pointer-events-none absolute right-2 size-3.5"
                          aria-hidden="true"
                        />
                      ) : null}
                    </button>
                  );
                })}
              </div>
            </div>,
            popoverRoot,
          )
        : null}
    </div>
  );
}
