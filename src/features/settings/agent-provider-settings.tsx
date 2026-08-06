import {
  useEffect,
  useRef,
  useState,
  type FormEvent,
} from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import {
  AlertCircleIcon,
  CheckCircle2Icon,
  ExternalLinkIcon,
  FolderOpenIcon,
  KeyRoundIcon,
  LogInIcon,
  RefreshCwIcon,
  Trash2Icon,
} from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/reui/alert";
import { Badge } from "@/components/reui/badge";
import {
  Accordion,
  AccordionContent,
  AccordionItem,
  AccordionTrigger,
} from "@/components/ui/accordion";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Card, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import {
  Field,
  FieldDescription,
  FieldError,
  FieldGroup,
  FieldLabel,
} from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Spinner } from "@/components/ui/spinner";
import {
  agentConfigurationClient,
  type AgentConfigurationClient,
  type AgentConfigurationError,
  type AgentConfigurationStatus,
  type ProviderConfigurationStatus,
  type SubscriptionLoginProgress,
  type SubscriptionLoginStage,
} from "@/lib/api/agent-configuration";
import type { TranslationKey } from "@/lib/i18n/resources";

type BusyAction = "reload" | "folder" | "api-key" | "login" | "remove";
type BusyState = { action: BusyAction; providerId?: string } | null;

export function AgentProviderSettings({
  client = agentConfigurationClient,
}: {
  client?: AgentConfigurationClient;
}) {
  const { t } = useTranslation();
  const [status, setStatus] = useState<AgentConfigurationStatus | null>(null);
  const [busy, setBusy] = useState<BusyState>(null);
  const [errorCode, setErrorCode] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [progress, setProgress] = useState<SubscriptionLoginProgress | null>(null);
  const loginAttemptRef = useRef<string | null>(null);
  const cancelledLoginAttemptsRef = useRef(new Set<string>());
  const [removeProvider, setRemoveProvider] =
    useState<ProviderConfigurationStatus | null>(null);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;

    void client
      .getStatus()
      .then((nextStatus) => {
        if (active) setStatus(nextStatus);
      })
      .catch((error: AgentConfigurationError) => {
        if (active) setErrorCode(error.code ?? "unavailable");
      });

    void client
      .listenToSubscriptionLoginProgress(
        (nextProgress) => {
          if (
            active &&
            loginAttemptRef.current === nextProgress.attemptId &&
            !cancelledLoginAttemptsRef.current.has(nextProgress.attemptId)
          ) {
            setProgress(nextProgress);
          }
        },
        () => {
          if (active) setErrorCode("transport_unavailable");
        },
      )
      .then((stopListening) => {
        if (!active) {
          stopListening();
          return;
        }
        unlisten = stopListening;
      })
      .catch(() => {
        // Status and non-login actions remain usable if event registration fails.
      });

    return () => {
      active = false;
      loginAttemptRef.current = null;
      cancelledLoginAttemptsRef.current.clear();
      unlisten?.();
    };
  }, [client]);

  const runStatusAction = async (
    nextBusy: NonNullable<BusyState>,
    action: () => Promise<AgentConfigurationStatus>,
    successNotice?: string,
    isCurrent: () => boolean = () => true,
  ) => {
    if (busy) return;
    setBusy(nextBusy);
    setErrorCode(null);
    setNotice(null);
    try {
      const nextStatus = await action();
      if (isCurrent()) {
        setStatus(nextStatus);
        if (successNotice) setNotice(successNotice);
      }
    } catch (error) {
      if (isCurrent()) {
        const configurationError = error as AgentConfigurationError;
        setErrorCode(configurationError.code ?? "unavailable");
      }
    } finally {
      if (isCurrent()) setBusy(null);
    }
  };

  const handleOpenFolder = async () => {
    if (busy) return;
    setBusy({ action: "folder" });
    setErrorCode(null);
    try {
      await client.openDataFolder();
    } catch (error) {
      setErrorCode((error as AgentConfigurationError).code ?? "unavailable");
    } finally {
      setBusy(null);
    }
  };

  const handleLogin = async (providerId: string) => {
    const attemptId = crypto.randomUUID();
    loginAttemptRef.current = attemptId;
    cancelledLoginAttemptsRef.current.delete(attemptId);
    setProgress({ attemptId, providerId, stage: "starting" });
    await runStatusAction(
      { action: "login", providerId },
      () => client.loginSubscription(providerId, attemptId),
      t("settings.agents.notices.loginComplete"),
      () =>
        loginAttemptRef.current === attemptId &&
        !cancelledLoginAttemptsRef.current.has(attemptId),
    );
  };

  const handleCancelLogin = async (providerId: string) => {
    const attempt =
      progress?.providerId === providerId &&
      progress.attemptId === loginAttemptRef.current
        ? progress
        : null;
    if (!attempt) return;
    try {
      await client.cancelSubscriptionLogin(attempt.attemptId);
      if (loginAttemptRef.current === attempt.attemptId) {
        cancelledLoginAttemptsRef.current.add(attempt.attemptId);
        setProgress({ ...attempt, stage: "cancelled" });
        setBusy(null);
      }
    } catch (error) {
      if (loginAttemptRef.current === attempt.attemptId) {
        setErrorCode((error as AgentConfigurationError).code ?? "unavailable");
      }
    }
  };

  const handleRemove = async () => {
    if (!removeProvider) return;
    const provider = removeProvider;
    setRemoveProvider(null);
    await runStatusAction(
      { action: "remove", providerId: provider.id },
      () => client.removeAuthentication(provider.id),
      t("settings.agents.notices.authenticationRemoved"),
    );
  };

  const diagnosticLabels = status?.diagnostics.map((diagnostic) =>
    diagnosticLabel(diagnostic.code, t),
  );

  return (
    <Card>
      <CardHeader className="gap-4 px-0 pt-0">
        <div className="flex flex-col gap-1.5">
          <CardTitle>{t("settings.agents.title")}</CardTitle>
          <CardDescription>{t("settings.agents.description")}</CardDescription>
        </div>
        <div className="flex flex-wrap gap-2">
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy !== null || status === null}
            onClick={() =>
              void runStatusAction(
                { action: "reload" },
                () => client.reload(),
                t("settings.agents.notices.reloaded"),
              )
            }
          >
            {busy?.action === "reload" ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <RefreshCwIcon data-icon="inline-start" />
            )}
            {t("settings.agents.actions.reload")}
          </Button>
          <Button
            type="button"
            variant="outline"
            size="sm"
            disabled={busy !== null}
            onClick={() => void handleOpenFolder()}
          >
            <FolderOpenIcon data-icon="inline-start" />
            {t("settings.agents.actions.openFolder")}
          </Button>
        </div>
      </CardHeader>

      <div aria-live="polite" className="flex flex-col gap-3">
        {errorCode ? (
          <Alert variant="destructive">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>{t("settings.agents.errors.title")}</AlertTitle>
            <AlertDescription>{errorLabel(errorCode, t)}</AlertDescription>
          </Alert>
        ) : null}
        {notice ? (
          <Alert variant="success">
            <CheckCircle2Icon aria-hidden="true" />
            <AlertTitle>{t("settings.agents.notices.title")}</AlertTitle>
            <AlertDescription>{notice}</AlertDescription>
          </Alert>
        ) : null}
        {diagnosticLabels?.length ? (
          <Alert variant="warning">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>{t("settings.agents.diagnostics.title")}</AlertTitle>
            <AlertDescription>
              <ul className="flex list-disc flex-col gap-1 pl-4">
                {diagnosticLabels.map((label, index) => (
                  <li key={`${label}-${index}`}>{label}</li>
                ))}
              </ul>
            </AlertDescription>
          </Alert>
        ) : null}
      </div>

      {status ? (
        <Accordion className="mt-4" multiple>
          {status.providers.map((provider) => (
            <ProviderRow
              key={provider.id}
              provider={provider}
              busy={busy}
              progress={progress?.providerId === provider.id ? progress : null}
              onSubmitApiKey={(apiKey) =>
                runStatusAction(
                  { action: "api-key", providerId: provider.id },
                  () => client.submitApiKey(provider.id, apiKey),
                  t("settings.agents.notices.apiKeySaved"),
                )
              }
              onLogin={() => handleLogin(provider.id)}
              onCancelLogin={() => handleCancelLogin(provider.id)}
              onRequestRemove={() => setRemoveProvider(provider)}
            />
          ))}
        </Accordion>
      ) : (
        <p className="mt-4 text-xs text-muted-foreground" role="status">
          {t("settings.agents.loading")}
        </p>
      )}

      <AlertDialog
        open={removeProvider !== null}
        onOpenChange={(open) => {
          if (!open) setRemoveProvider(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>
              {t("settings.agents.removeDialog.title")}
            </AlertDialogTitle>
            <AlertDialogDescription>
              {t("settings.agents.removeDialog.description", {
                provider: removeProvider?.displayName ?? "",
              })}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>{t("settings.agents.actions.cancel")}</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={() => void handleRemove()}
            >
              {t("settings.agents.actions.remove")}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </Card>
  );
}

function ProviderRow({
  provider,
  busy,
  progress,
  onSubmitApiKey,
  onLogin,
  onCancelLogin,
  onRequestRemove,
}: {
  provider: ProviderConfigurationStatus;
  busy: BusyState;
  progress: SubscriptionLoginProgress | null;
  onSubmitApiKey: (apiKey: string) => Promise<void>;
  onLogin: () => Promise<void>;
  onCancelLogin: () => Promise<void>;
  onRequestRemove: () => void;
}) {
  const { t } = useTranslation();
  const providerBusy = busy?.providerId === provider.id;
  const loginBusy = providerBusy && busy?.action === "login";

  return (
    <AccordionItem value={provider.id}>
      <AccordionTrigger>
        <span className="flex min-w-0 flex-1 flex-wrap items-center gap-2">
          <span className="truncate">{provider.displayName}</span>
          <Badge
            size="sm"
            variant={
              provider.capability === "executable"
                ? "success-light"
                : provider.capability === "configured_only"
                  ? "warning-light"
                  : "secondary"
            }
          >
            {provider.capability === "executable"
              ? t("settings.agents.status.executable")
              : provider.capability === "configured_only"
                ? t("settings.agents.status.configuredOnly")
                : t("settings.agents.status.catalogOnly")}
          </Badge>
          <span className="text-muted-foreground">
            {t("settings.agents.modelCount", { count: provider.models.length })}
          </span>
        </span>
      </AccordionTrigger>
      <AccordionContent className="flex flex-col gap-4">
        {provider.capability === "configured_only" ? (
          <Alert variant="warning">
            <AlertCircleIcon aria-hidden="true" />
            <AlertTitle>{t("settings.agents.status.configuredOnly")}</AlertTitle>
            <AlertDescription>
              {t("settings.agents.configuredOnlyDescription")}
            </AlertDescription>
          </Alert>
        ) : null}

        {provider.authenticationMethods.includes("subscription") ? (
          <section className="flex flex-col gap-2" aria-labelledby={`${provider.id}-subscription`}>
            <div className="flex flex-wrap items-start justify-between gap-2">
              <div className="flex flex-col gap-0.5">
                <h3 id={`${provider.id}-subscription`} className="text-xs font-medium">
                  {t("settings.agents.subscription.title")}
                </h3>
                <p className="text-xs text-muted-foreground">
                  {t("settings.agents.subscription.description")}
                </p>
              </div>
              {loginBusy ? (
                <Button type="button" size="sm" variant="outline" onClick={() => void onCancelLogin()}>
                  {t("settings.agents.actions.cancelLogin")}
                </Button>
              ) : (
                <Button type="button" size="sm" disabled={busy !== null} onClick={() => void onLogin()}>
                  {provider.activeAuthentication === "subscription" ? (
                    <ExternalLinkIcon data-icon="inline-start" />
                  ) : (
                    <LogInIcon data-icon="inline-start" />
                  )}
                  {provider.activeAuthentication === "subscription"
                    ? t("settings.agents.actions.replaceSubscription")
                    : t("settings.agents.actions.login")}
                </Button>
              )}
            </div>
            {loginBusy && progress ? (
              <p className="flex items-center gap-2 text-xs text-muted-foreground" role="status" aria-live="polite">
                <Spinner />
                {progressLabel(progress.stage, t)}
              </p>
            ) : null}
          </section>
        ) : null}

        {provider.authenticationMethods.includes("api_key") ? (
          <ProviderApiKeyForm
            providerId={provider.id}
            activeAuthentication={provider.activeAuthentication}
            busy={busy !== null}
            onSubmitApiKey={(_providerId, apiKey) => onSubmitApiKey(apiKey)}
          />
        ) : null}

        {provider.activeAuthentication ? (
          <div>
            <Button type="button" size="sm" variant="destructive" disabled={busy !== null} onClick={onRequestRemove}>
              <Trash2Icon data-icon="inline-start" />
              {provider.activeAuthentication === "subscription"
                ? t("settings.agents.actions.logout")
                : t("settings.agents.actions.removeApiKey")}
            </Button>
          </div>
        ) : null}
      </AccordionContent>
    </AccordionItem>
  );
}

export function ProviderApiKeyForm({
  providerId,
  activeAuthentication,
  busy,
  onSubmitApiKey,
}: {
  providerId: string;
  activeAuthentication: ProviderConfigurationStatus["activeAuthentication"];
  busy: boolean;
  onSubmitApiKey: (providerId: string, apiKey: string) => Promise<void>;
}) {
  const { t } = useTranslation();
  const apiKeyInput = useRef<HTMLInputElement>(null);
  const [apiKeyInvalid, setApiKeyInvalid] = useState(false);

  useEffect(() => () => {
    if (apiKeyInput.current) apiKeyInput.current.value = "";
  }, []);

  const handleSubmit = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const input = apiKeyInput.current;
    if (!input) return;
    const apiKey = input.value;
    input.value = "";
    if (!apiKey.trim()) {
      setApiKeyInvalid(true);
      input.focus();
      return;
    }
    setApiKeyInvalid(false);
    void onSubmitApiKey(providerId, apiKey);
  };

  return (
    <form className="flex flex-col gap-2" onSubmit={handleSubmit}>
      <FieldGroup>
        <Field data-invalid={apiKeyInvalid || undefined}>
          <FieldLabel htmlFor={`${providerId}-api-key`}>
            {t("settings.agents.apiKey.title")}
          </FieldLabel>
          <FieldDescription id={`${providerId}-api-key-description`}>
            {activeAuthentication === "api_key"
              ? t("settings.agents.apiKey.replaceDescription")
              : t("settings.agents.apiKey.description")}
          </FieldDescription>
          <Input
            ref={apiKeyInput}
            id={`${providerId}-api-key`}
            name="apiKey"
            type="password"
            autoComplete="off"
            spellCheck={false}
            aria-invalid={apiKeyInvalid}
            aria-describedby={
              apiKeyInvalid
                ? `${providerId}-api-key-description ${providerId}-api-key-error`
                : `${providerId}-api-key-description`
            }
            disabled={busy}
          />
          {apiKeyInvalid ? (
            <FieldError id={`${providerId}-api-key-error`}>
              {t("settings.agents.apiKey.required")}
            </FieldError>
          ) : null}
        </Field>
      </FieldGroup>
      <div>
        <Button type="submit" size="sm" variant="outline" disabled={busy}>
          <KeyRoundIcon data-icon="inline-start" />
          {activeAuthentication === "api_key"
            ? t("settings.agents.actions.replaceApiKey")
            : t("settings.agents.actions.saveApiKey")}
        </Button>
      </div>
    </form>
  );
}

type Translator = TFunction<"translation">;

function progressLabel(stage: SubscriptionLoginStage, t: Translator) {
  const keys: Record<SubscriptionLoginStage, TranslationKey> = {
    starting: "settings.agents.progress.starting",
    opening_browser: "settings.agents.progress.openingBrowser",
    waiting_for_browser: "settings.agents.progress.waitingForBrowser",
    displaying_device_code: "settings.agents.progress.displayingDeviceCode",
    finalizing: "settings.agents.progress.finalizing",
    completed: "settings.agents.progress.completed",
    cancelled: "settings.agents.progress.cancelled",
    failed: "settings.agents.progress.failed",
  };
  return t(keys[stage]);
}

function diagnosticLabel(code: string, t: Translator) {
  if (code === "authentication_configuration_invalid") {
    return t("settings.agents.diagnostics.authenticationInvalid");
  }
  if (code === "model_configuration_invalid") {
    return t("settings.agents.diagnostics.modelsInvalid");
  }
  return t("settings.agents.diagnostics.unknown");
}

function errorLabel(code: string, t: Translator) {
  if (code.includes("cancel")) return t("settings.agents.errors.cancelled");
  if (code.includes("api_key")) return t("settings.agents.errors.apiKey");
  if (code.includes("login")) return t("settings.agents.errors.login");
  return t("settings.agents.errors.unavailable");
}
