import { useEffect, useMemo, useState } from "react";

import { AlertCircleIcon, InfoIcon } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Field,
  FieldDescription,
  FieldGroup,
  FieldLabel,
  FieldLegend,
  FieldSet,
} from "@/components/ui/field";
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { LocationListEditor } from "@/features/search-requests/components/location-list-editor";
import { SearchRuleEditor } from "@/features/search-requests/components/search-rule-editor";
import { SourceKeyPicker } from "@/features/search-requests/components/source-key-picker";
import {
  buildSearchRequestInput,
  emptySearchRequestForm,
  searchRequestFormFromRequest,
  type SearchRequestFormState,
} from "@/features/search-requests/model/search-request-form-model";
import { searchRequestStatusOptions } from "@/features/search-requests/status";
import type {
  CreateSearchRequestInput,
  SearchRequest,
  SearchRequestStatus,
  UpdateSearchRequestInput,
} from "@/lib/api/search-requests";
import type { RegistrySource } from "@/lib/api/sources";

type SearchRequestFormDialogProps = {
  open: boolean;
  request: SearchRequest | null;
  sources: RegistrySource[];
  onOpenChange: (open: boolean) => void;
  onSubmit: (
    input: CreateSearchRequestInput | UpdateSearchRequestInput,
    request: SearchRequest | null,
  ) => Promise<void>;
};

export function SearchRequestFormDialog({
  open,
  request,
  sources,
  onOpenChange,
  onSubmit,
}: SearchRequestFormDialogProps) {
  const [form, setForm] = useState<SearchRequestFormState>(emptySearchRequestForm);
  const [saveAttempted, setSaveAttempted] = useState(false);
  const [pending, setPending] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    setForm(request ? searchRequestFormFromRequest(request) : emptySearchRequestForm);
    setSaveAttempted(false);
    setSubmitError(null);
  }, [open, request]);

  const buildResult = useMemo(() => buildSearchRequestInput(form), [form]);
  const title = request ? "Search Request bearbeiten" : "Search Request erstellen";
  const disabled = pending;

  const handleSubmit = async () => {
    setSaveAttempted(true);
    setSubmitError(null);
    const result = buildSearchRequestInput(form);
    if (!result.input) return;

    try {
      setPending(true);
      await onSubmit(result.input, request);
      onOpenChange(false);
    } catch (unknownError) {
      setSubmitError(errorMessage(unknownError));
    } finally {
      setPending(false);
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(nextOpen) => {
        if (pending) return;
        onOpenChange(nextOpen);
      }}
    >
      <DialogContent className="max-h-[calc(100vh-2rem)] overflow-y-auto sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>
            Suchkriterien gehören zur Search Request. Sources werden über stabile Source Keys aus der Source Registry referenziert.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-5">
          <Alert variant="info">
            <InfoIcon aria-hidden="true" />
            <AlertTitle>Matching-Verhalten</AlertTitle>
            <AlertDescription>
              Exclude-Regex-Regeln werden case-insensitive geprüft. Include-Regex-Regeln bleiben unverändert und nutzen die Regex so, wie du sie eingibst.
            </AlertDescription>
          </Alert>

          {request?.validationError ? (
            <Alert variant="warning">
              <AlertCircleIcon aria-hidden="true" />
              <AlertTitle>Backend-Validierung</AlertTitle>
              <AlertDescription>{request.validationError}</AlertDescription>
            </Alert>
          ) : null}

          <FieldSet>
            <FieldLegend>Status</FieldLegend>
            <FieldGroup>
              <Field>
                <FieldLabel>Status</FieldLabel>
                <Select
                  items={searchRequestStatusOptions}
                  modal={false}
                  disabled={disabled}
                  value={form.status}
                  onValueChange={(value) => {
                    if (!value) return;
                    setForm((current) => ({
                      ...current,
                      status: value as SearchRequestStatus,
                    }));
                  }}
                >
                  <SelectTrigger className="w-full" aria-label="Status wählen">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent alignItemWithTrigger={false}>
                    <SelectGroup>
                      {searchRequestStatusOptions.map(({ value, label }) => (
                        <SelectItem key={value} value={value}>
                          {label}
                        </SelectItem>
                      ))}
                    </SelectGroup>
                  </SelectContent>
                </Select>
                <FieldDescription>
                  Aktive Search Requests brauchen mindestens eine Include-Regel und eine Source.
                </FieldDescription>
              </Field>
            </FieldGroup>
          </FieldSet>

          <SearchRuleEditor
            title="Include-Regeln"
            description="Mindestens eine Include-Regel macht eine aktive Search Request ausführbar. Ziel ist aktuell immer der Titel."
            rules={form.includeRules}
            disabled={disabled}
            onChange={(includeRules) => setForm((current) => ({ ...current, includeRules }))}
          />

          <SearchRuleEditor
            title="Exclude-Regeln"
            description="Exclude-Regeln entfernen bereits gefundene Treffer wieder. Regex-Exclude-Regeln sind case-insensitive."
            rules={form.excludeRules}
            disabled={disabled}
            onChange={(excludeRules) => setForm((current) => ({ ...current, excludeRules }))}
          />

          <LocationListEditor
            locationsText={form.locationsText}
            radiusKmText={form.radiusKmText}
            disabled={disabled}
            onLocationsTextChange={(locationsText) =>
              setForm((current) => ({ ...current, locationsText }))
            }
            onRadiusKmTextChange={(radiusKmText) =>
              setForm((current) => ({ ...current, radiusKmText }))
            }
          />

          <SourceKeyPicker
            sources={sources}
            selectedSourceKeys={form.sourceKeys}
            disabled={disabled}
            onChange={(sourceKeys) => setForm((current) => ({ ...current, sourceKeys }))}
          />
        </div>

        {(saveAttempted && buildResult.errors.length) || submitError ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>Search Request noch nicht speicherbar</AlertTitle>
            <AlertDescription>
              {submitError ? <p>{submitError}</p> : null}
              {saveAttempted && buildResult.errors.length ? (
                <ul className="list-inside list-disc">
                  {buildResult.errors.map((error) => (
                    <li key={error}>{error}</li>
                  ))}
                </ul>
              ) : null}
            </AlertDescription>
          </Alert>
        ) : null}

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={pending}
          >
            Abbrechen
          </Button>
          <Button type="button" onClick={() => void handleSubmit()} disabled={pending}>
            {pending ? <Spinner data-icon="inline-start" /> : null}
            {request ? "Änderungen speichern" : "Search Request erstellen"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
