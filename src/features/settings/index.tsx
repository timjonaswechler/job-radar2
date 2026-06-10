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
import { Input } from "@/components/ui/input";
import {
  getAppPreferences,
  setDefaultSearchRadiusKm,
  type AppPreferences,
} from "@/lib/api/app-preferences";

const maxSearchRadiusKm = 500;

export function SettingsFeature() {
  const [preferences, setPreferences] = useState<AppPreferences | null>(null);
  const [radiusText, setRadiusText] = useState("30");
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

    const radiusKm = Number(radiusText);
    if (!Number.isInteger(radiusKm) || radiusKm < 0 || radiusKm > maxSearchRadiusKm) {
      setError(`Der Standard-Suchradius muss zwischen 0 und ${maxSearchRadiusKm} km liegen.`);
      setSaved(false);
      return;
    }

    try {
      setSaving(true);
      setError(null);
      const nextPreferences = await setDefaultSearchRadiusKm(radiusKm);
      setPreferences(nextPreferences);
      setRadiusText(String(nextPreferences.defaultSearchRadiusKm));
      setSaved(true);
    } catch (unknownError) {
      setError(String(unknownError));
      setSaved(false);
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="grid gap-5">
      <Frame>
        <FramePanel>
          <FrameHeader className="gap-2">
            <div className="flex items-center gap-2">
              <SettingsIcon className="size-5 text-muted-foreground" aria-hidden="true" />
              <FrameTitle>Einstellungen</FrameTitle>
            </div>
            <FrameDescription>
              Globale Vorgaben für Suchläufe. Quellen speichern weiterhin nur stabile Portal- und Zugriffskonfiguration.
            </FrameDescription>
          </FrameHeader>
        </FramePanel>
      </Frame>

      {error ? (
        <Alert variant="destructive">
          <AlertCircleIcon className="size-4" aria-hidden="true" />
          <AlertTitle>Einstellungen konnten nicht gespeichert werden</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {saved ? (
        <Alert variant="success">
          <CheckCircle2Icon className="size-4" aria-hidden="true" />
          <AlertTitle>Einstellungen gespeichert</AlertTitle>
          <AlertDescription>
            Neuer Standard-Suchradius: {preferences?.defaultSearchRadiusKm ?? radiusText} km.
          </AlertDescription>
        </Alert>
      ) : null}

      <Frame>
        <FramePanel>
          <form className="grid max-w-xl gap-4" onSubmit={(event) => void handleSubmit(event)}>
            <FrameHeader className="gap-1 px-0 pt-0">
              <FrameTitle>Job-Portal-Suchläufe</FrameTitle>
              <FrameDescription>
                Der Radius gilt als Standard für StepStone- und Indeed-Suchläufe. Suchtext und Ort werden erst im Suchlauf gesetzt.
              </FrameDescription>
            </FrameHeader>

            <div className="grid gap-1.5">
              <label className="text-xs font-medium" htmlFor="default-search-radius-km">
                Standard-Suchradius in km
              </label>
              <p className="text-xs text-muted-foreground">
                Wird später in internen URL-Templates als {"{radiusKm}"} eingesetzt. Nicht in einzelnen Quellen speichern.
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
