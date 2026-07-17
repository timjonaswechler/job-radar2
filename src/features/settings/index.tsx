import { useEffect, useState, type FormEvent } from "react";

import { AlertCircleIcon, CheckCircle2Icon, SettingsIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import {
  Frame,
  FrameDescription,
  FrameHeader,
  FramePanel,
  FrameTitle,
} from "@/components/reui/frame";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  getAppPreferences,
  setBaseFontSizePx,
  setDefaultSearchRadiusKm,
  setWindowDragRegionEnabled as setWindowDragRegionEnabledPreference,
  type AppPreferences,
} from "@/lib/api/app-preferences";
import { APP_SETTINGS, isBaseFontSizePx } from "@/lib/app-settings";
import {
  applyBaseFontSizeToDocument,
  writeStoredBaseFontSizePx,
} from "@/lib/font-size";
import { applyStoredWindowDragRegionEnabled } from "@/lib/window-chrome";
import { AgentProviderSettings } from "@/features/settings/agent-provider-settings";

const maxSearchRadiusKm = 500;

export function SettingsFeature() {
  const [preferences, setPreferences] = useState<AppPreferences | null>(null);
  const [radiusText, setRadiusText] = useState("30");
  const [baseFontSizeText, setBaseFontSizeText] = useState(
    String(APP_SETTINGS.baseFontSizePx.default),
  );
  const [windowDragRegionEnabled, setWindowDragRegionEnabled] = useState(
    APP_SETTINGS.windowDragRegionEnabled.default,
  );
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    let cancelled = false;

    void getAppPreferences()
      .then((nextPreferences) => {
        if (cancelled) return;
        setPreferences(nextPreferences);
        setRadiusText(String(nextPreferences.defaultSearchRadiusKm));
        setBaseFontSizeText(String(nextPreferences.baseFontSizePx));
        setWindowDragRegionEnabled(nextPreferences.windowDragRegionEnabled);
        writeStoredBaseFontSizePx(nextPreferences.baseFontSizePx);
        applyBaseFontSizeToDocument(nextPreferences.baseFontSizePx);
        applyStoredWindowDragRegionEnabled(
          nextPreferences.windowDragRegionEnabled,
        );
        setError(null);
      })
      .catch((unknownError) => {
        if (cancelled) return;
        setError(String(unknownError));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, []);

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    if (loading || saving || !preferences) return;

    const radiusKm = Number(radiusText);
    if (
      !Number.isInteger(radiusKm) ||
      radiusKm < 0 ||
      radiusKm > maxSearchRadiusKm
    ) {
      setError(
        `Der Standard-Suchradius muss zwischen 0 und ${maxSearchRadiusKm} km liegen.`,
      );
      setSaved(false);
      return;
    }

    const baseFontSizePx = Number(baseFontSizeText);
    if (!isBaseFontSizePx(baseFontSizePx)) {
      setError(
        `Die Basisschriftgröße muss zwischen ${APP_SETTINGS.baseFontSizePx.min} und ${APP_SETTINGS.baseFontSizePx.max} px liegen.`,
      );
      setSaved(false);
      return;
    }

    try {
      setSaving(true);
      setError(null);
      const saveOperations: Promise<AppPreferences>[] = [];

      if (preferences.defaultSearchRadiusKm !== radiusKm) {
        saveOperations.push(setDefaultSearchRadiusKm(radiusKm));
      }
      if (preferences.baseFontSizePx !== baseFontSizePx) {
        saveOperations.push(setBaseFontSizePx(baseFontSizePx));
      }
      if (preferences.windowDragRegionEnabled !== windowDragRegionEnabled) {
        saveOperations.push(
          setWindowDragRegionEnabledPreference(windowDragRegionEnabled),
        );
      }

      if (saveOperations.length > 0) {
        await Promise.all(saveOperations);
      }

      const nextPreferences =
        saveOperations.length > 0 ? await getAppPreferences() : preferences;

      setPreferences(nextPreferences);
      setRadiusText(String(nextPreferences.defaultSearchRadiusKm));
      setBaseFontSizeText(String(nextPreferences.baseFontSizePx));
      setWindowDragRegionEnabled(nextPreferences.windowDragRegionEnabled);
      writeStoredBaseFontSizePx(nextPreferences.baseFontSizePx);
      applyBaseFontSizeToDocument(nextPreferences.baseFontSizePx);
      applyStoredWindowDragRegionEnabled(
        nextPreferences.windowDragRegionEnabled,
      );
      setSaved(true);
    } catch (unknownError) {
      setError(String(unknownError));
      setSaved(false);
    } finally {
      setSaving(false);
    }
  };

  const savedWindowDragRegionLabel =
    (preferences?.windowDragRegionEnabled ?? windowDragRegionEnabled)
      ? "an"
      : "aus";

  return (
    <div className="grid gap-4 p-2">
      <Frame>
        <FramePanel>
          <FrameHeader className="gap-2">
            <div className="flex items-center gap-2">
              <SettingsIcon
                className="size-5 text-muted-foreground"
                aria-hidden="true"
              />
              <FrameTitle>Einstellungen</FrameTitle>
            </div>
            <FrameDescription>
              Globale Vorgaben für Darstellung und Suchläufe. Quellen speichern
              weiterhin nur stabile Portal- und Zugriffskonfiguration.
            </FrameDescription>
          </FrameHeader>
        </FramePanel>
      </Frame>

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>
            Einstellungen konnten nicht gespeichert werden
          </AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {saved ? (
        <Alert variant="success">
          <CheckCircle2Icon className="size-4" aria-hidden="true" />
          <AlertTitle>Einstellungen gespeichert</AlertTitle>
          <AlertDescription>
            Neuer Standard-Suchradius:{" "}
            {preferences?.defaultSearchRadiusKm ?? radiusText} km.
            Basisschriftgröße: {preferences?.baseFontSizePx ?? baseFontSizeText}{" "}
            px. Drag-Bereich: {savedWindowDragRegionLabel}.
          </AlertDescription>
        </Alert>
      ) : null}

      <Frame>
        <FramePanel>
          <AgentProviderSettings />
        </FramePanel>
      </Frame>

      <Frame>
        <FramePanel>
          <form
            className="grid max-w-xl gap-4"
            onSubmit={(event) => void handleSubmit(event)}
          >
            <FrameHeader className="gap-1 px-0 pt-0">
              <FrameTitle>Globale Vorgaben</FrameTitle>
              <FrameDescription>
                Lege Darstellung und Standardwerte für Job-Portal-Suchläufe
                fest. Suchtext und Ort werden erst im Suchlauf gesetzt.
              </FrameDescription>
            </FrameHeader>

            <div className="grid gap-1.5">
              <label
                className="text-xs font-medium"
                htmlFor="default-search-radius-km"
              >
                Standard-Suchradius in km
              </label>
              <p className="text-xs text-muted-foreground">
                Wird später in internen URL-Templates als {"{radiusKm}"}{" "}
                eingesetzt. Nicht in einzelnen Quellen speichern.
              </p>
              <Input
                id="default-search-radius-km"
                type="number"
                min={0}
                max={maxSearchRadiusKm}
                step={1}
                value={radiusText}
                onChange={(event) => {
                  setRadiusText(event.target.value);
                  setSaved(false);
                }}
                disabled={loading || saving}
                required
              />
            </div>

            <div className="grid gap-1.5">
              <label
                className="text-xs font-medium"
                htmlFor="base-font-size-px"
              >
                Basisschriftgröße in px
              </label>
              <p className="text-xs text-muted-foreground">
                Setzt die Root-Schriftgröße. Rem-basierte Tailwind- und
                shadcn-Abstände skalieren dadurch mit.
              </p>
              <Input
                id="base-font-size-px"
                type="number"
                min={APP_SETTINGS.baseFontSizePx.min}
                max={APP_SETTINGS.baseFontSizePx.max}
                step={1}
                value={baseFontSizeText}
                onChange={(event) => {
                  setBaseFontSizeText(event.target.value);
                  setSaved(false);
                }}
                disabled={loading || saving}
                required
              />
            </div>

            <div className="flex items-start gap-2.5">
              <Checkbox
                id="window-drag-region-enabled"
                checked={windowDragRegionEnabled}
                onCheckedChange={(checked) => {
                  setWindowDragRegionEnabled(checked === true);
                  setSaved(false);
                }}
                disabled={loading || saving}
              />
              <div className="grid gap-1.5">
                <label
                  className="text-xs font-medium"
                  htmlFor="window-drag-region-enabled"
                >
                  Fenster über den oberen App-Bereich ziehen
                </label>
                <p className="text-xs text-muted-foreground">
                  Aktiviert die Tauri-Drag-Region im Header. Interaktive
                  Elemente bleiben klickbar; auf macOS wird zusätzlich Platz für
                  die native Ampel reserviert.
                </p>
              </div>
            </div>

            <div>
              <Button type="submit" size="sm" disabled={loading || saving}>
                {saving ? "Speichert…" : "Einstellungen speichern"}
              </Button>
            </div>
          </form>
        </FramePanel>
      </Frame>
    </div>
  );
}
