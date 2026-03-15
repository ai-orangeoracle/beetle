import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import FormControl from "@mui/material/FormControl";
import InputLabel from "@mui/material/InputLabel";
import MenuItem from "@mui/material/MenuItem";
import Select from "@mui/material/Select";
import Stack from "@mui/material/Stack";
import Button from "@mui/material/Button";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import type { SelectChangeEvent } from "@mui/material/Select";
import AddRounded from "@mui/icons-material/AddRounded";
import DeleteOutlined from "@mui/icons-material/DeleteOutlined";
import SaveRounded from "@mui/icons-material/SaveRounded";
import SmartToyOutlined from "@mui/icons-material/SmartToyOutlined";
import { ConfirmDialog } from "../components/ConfirmDialog";
import {
  FormFieldStack,
  FormLoadingSkeleton,
  FormSectionSubCollapsible,
  InlineAlert,
  SaveFeedback,
} from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { useConfig } from "../hooks/useConfig";
import { useToast } from "../hooks/useToast";
import { useUnsaved } from "../hooks/useUnsaved";
import { useRevealedPasswordFields } from "../hooks/useRevealedPassword";
import type { LlmSource } from "../types/appConfig";

const MAX_LEN = 64;
const MAX_API_URL = 256;

/** 后端支持的 LLM provider 取值，与 main.rs 中分支一致。 */
const LLM_PROVIDER_VALUES = ["anthropic", "openai", "openai_compatible"] as const;
const DEFAULT_PROVIDER: (typeof LLM_PROVIDER_VALUES)[number] = "openai_compatible";

type LlmProviderValue = (typeof LLM_PROVIDER_VALUES)[number];

function normalizeProvider(provider: string): LlmProviderValue {
  return (LLM_PROVIDER_VALUES as readonly string[]).includes(provider)
    ? (provider as LlmProviderValue)
    : DEFAULT_PROVIDER;
}

type SourceFormRow = LlmSource & { provider: LlmProviderValue };

function toSourceRows(sources: LlmSource[]): SourceFormRow[] {
  return sources.map((s) => ({
    ...s,
    provider: normalizeProvider(s.provider),
  }));
}

function validateSources(
  rows: SourceFormRow[],
  routerIndex: number | null,
  workerIndex: number | null,
  t: (k: string) => string,
): string | null {
  if (rows.length === 0) return t("config.validation.llmSourcesNonEmpty");
  const n = rows.length;
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    if (r.provider.length > MAX_LEN) return t("config.validation.fieldMax64");
    if (r.api_key.length > MAX_LEN) return t("config.validation.fieldMax64");
    if (r.model.length > MAX_LEN) return t("config.validation.fieldMax64");
    if (r.api_url.length > MAX_API_URL)
      return t("config.validation.apiUrlMax256");
  }
  if (routerIndex != null && (routerIndex < 0 || routerIndex >= n))
    return t("config.validation.routerIndexInRange");
  if (workerIndex != null && (workerIndex < 0 || workerIndex >= n))
    return t("config.validation.workerIndexInRange");
  return null;
}

export function AIConfigPage() {
  const { t } = useTranslation();
  const { config, loadConfig, saveLlm, loading, error } = useConfig();
  const { showToast } = useToast();
  const { setDirty } = useUnsaved();
  const [sources, setSources] = useState<SourceFormRow[]>([]);
  const [routerIndex, setRouterIndex] = useState<number | null>(null);
  const [workerIndex, setWorkerIndex] = useState<number | null>(null);
  const [saveStatus, setSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [saveError, setSaveError] = useState("");
  const [removeSourceIndex, setRemoveSourceIndex] = useState<number | null>(null);
  const loadAttemptedRef = useRef(false);
  const { isRevealed, getRevealHandlers } = useRevealedPasswordFields();

  useEffect(() => {
    if (config !== null) {
      loadAttemptedRef.current = false;
      return;
    }
    if (loading || loadAttemptedRef.current) return;
    loadAttemptedRef.current = true;
    loadConfig();
  }, [config, loading, loadConfig]);

  useEffect(() => {
    if (!config) {
      queueMicrotask(() => {
        setSources([]);
        setRouterIndex(null);
        setWorkerIndex(null);
      });
      return;
    }
    const list =
      config.llm_sources?.length > 0
        ? config.llm_sources
        : [
            {
              provider: config.model_provider || "",
              api_key: config.api_key || "",
              model: config.model || "",
              api_url: config.api_url || "",
            },
          ];
    const sync = () => {
      setSources(toSourceRows(list));
      setRouterIndex(config.llm_router_source_index ?? null);
      setWorkerIndex(config.llm_worker_source_index ?? null);
    };
    queueMicrotask(sync);
  }, [config]);

  const addSource = () => {
    setDirty(true);
    setSources((prev) => [
      ...prev,
      {
        provider: DEFAULT_PROVIDER,
        api_key: "",
        model: "",
        api_url: "",
      },
    ]);
  };

  const requestRemoveSource = (i: number) => {
    if (sources.length <= 1) return;
    setRemoveSourceIndex(i);
  };
  const confirmRemoveSource = () => {
    if (removeSourceIndex == null) return;
    setDirty(true);
    setSources((prev) => prev.filter((_, j) => j !== removeSourceIndex));
    setRemoveSourceIndex(null);
  };

  const updateSource = (i: number, field: keyof LlmSource, value: string) => {
    setDirty(true);
    setSources((prev) => {
      const next = [...prev];
      next[i] = { ...next[i], [field]: value };
      return next;
    });
  };

  const handleSave = async () => {
    if (!config) return;
    const err = validateSources(sources, routerIndex, workerIndex, t);
    if (err) {
      setSaveStatus("fail");
      setSaveError(err);
      return;
    }
    const llm_sources: LlmSource[] = sources.map((r) => ({
      provider: r.provider.trim(),
      api_key: r.api_key.trim(),
      model: r.model.trim(),
      api_url: r.api_url.trim(),
    }));
    setSaveStatus("saving");
    setSaveError("");
    const result = await saveLlm({
      llm_sources,
      llm_router_source_index: routerIndex,
      llm_worker_source_index: workerIndex,
    });
    setSaveStatus(result.ok ? "ok" : "fail");
    const errMsg =
      result.error &&
      (result.error.startsWith("device.") || result.error.startsWith("config."))
        ? t(result.error)
        : (result.error ?? "");
    setSaveError(errMsg);
    if (result.ok) setDirty(false);
    showToast(result.ok ? t("common.saveOk") : errMsg, {
      variant: result.ok ? "success" : "error",
    });
  };

  if (loading && !config) {
    return (
      <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
        <SettingsSection
          icon={<SmartToyOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
          label={t("config.sectionLlm")}
        >
          <FormLoadingSkeleton />
        </SettingsSection>
      </Box>
    );
  }

  const saveDisabled = !config || saveStatus === "saving";

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={error} onRetry={loadConfig} />
      <ConfirmDialog
        open={removeSourceIndex != null}
        onClose={() => setRemoveSourceIndex(null)}
        title={t("config.llmRemoveSourceConfirmTitle")}
        description={t("config.llmRemoveSourceConfirmDesc")}
        icon={<DeleteOutlined />}
        confirmColor="error"
        confirmLabel={t("common.remove")}
        onConfirm={confirmRemoveSource}
      />
      <SettingsSection
        icon={<SmartToyOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("config.sectionLlm")}
        description={t("config.sectionLlmDesc")}
        accessory={
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveRounded />}
            onClick={handleSave}
            disabled={saveDisabled}
            title={!config ? t("config.hintSaveNeedDevice") : undefined}
            sx={{ borderRadius: "var(--radius-control)" }}
          >
            {saveStatus === "saving" ? t("common.saving") : t("common.save")}
          </Button>
        }
      >
        {!config && !loading && (
          <Typography variant="body2" color="text.secondary" sx={{ pb: 2 }}>
            {t("config.hintSaveNeedDevice")}
          </Typography>
        )}
        <Stack spacing={0}>
          {sources.map((row, i) => (
            <FormSectionSubCollapsible
              key={i}
              title={`${t("config.llmSource")} ${i + 1}`}
              defaultOpen={i === 0}
              action={
                <Box
                  component="span"
                  role="button"
                  tabIndex={0}
                  onClick={(e) => {
                    e.stopPropagation();
                    requestRemoveSource(i);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      requestRemoveSource(i);
                    }
                  }}
                  sx={{
                    display: "inline-flex",
                    alignItems: "center",
                    justifyContent: "center",
                    p: 0.5,
                    color: "var(--muted)",
                    cursor: sources.length <= 1 ? "default" : "pointer",
                    opacity: sources.length <= 1 ? 0.5 : 1,
                    borderRadius: "var(--radius-control)",
                    "&:focus-visible": {
                      outline: "2px solid var(--primary)",
                      outlineOffset: 2,
                    },
                  }}
                  aria-label={t("common.remove")}
                  aria-disabled={sources.length <= 1}
                >
                  <DeleteOutlined fontSize="small" />
                </Box>
              }
            >
              <FormFieldStack>
                <FormControl size="small" fullWidth>
                  <InputLabel id={`llm-provider-${i}`}>{t("config.llmProvider")}</InputLabel>
                  <Select
                    labelId={`llm-provider-${i}`}
                    label={t("config.llmProvider")}
                    value={normalizeProvider(row.provider)}
                    onChange={(e: SelectChangeEvent<string>) =>
                      updateSource(i, "provider", e.target.value)
                    }
                  >
                    <MenuItem value="anthropic">{t("config.providerAnthropic")}</MenuItem>
                    <MenuItem value="openai">{t("config.providerOpenai")}</MenuItem>
                    <MenuItem value="openai_compatible">
                      {t("config.providerOpenaiCompatible")}
                    </MenuItem>
                  </Select>
                </FormControl>
                <TextField
                  label={t("config.llmApiKey")}
                  value={row.api_key}
                  onChange={(e) => updateSource(i, "api_key", e.target.value)}
                  type={isRevealed(`api_key_${i}`) ? "text" : "password"}
                  size="small"
                  fullWidth
                  slotProps={{
                    htmlInput: {
                      maxLength: MAX_LEN,
                      style: { fontFamily: "var(--font-mono)" },
                      ...getRevealHandlers(`api_key_${i}`),
                    },
                  }}
                />
                <TextField
                  label={t("config.llmModel")}
                  value={row.model}
                  onChange={(e) => updateSource(i, "model", e.target.value)}
                  size="small"
                  fullWidth
                  slotProps={{ htmlInput: { maxLength: MAX_LEN } }}
                />
                <TextField
                  label={t("config.llmApiUrl")}
                  value={row.api_url}
                  onChange={(e) => updateSource(i, "api_url", e.target.value)}
                  size="small"
                  fullWidth
                  placeholder={t("config.placeholderApiUrl")}
                  slotProps={{
                    htmlInput: {
                      maxLength: MAX_API_URL,
                      style: { fontFamily: "var(--font-mono)" },
                    },
                  }}
                />
              </FormFieldStack>
            </FormSectionSubCollapsible>
          ))}
          <Button
            startIcon={<AddRounded />}
            onClick={addSource}
            variant="outlined"
            size="small"
            sx={{
              mt: 2,
              alignSelf: "flex-start",
              borderRadius: "var(--radius-control)",
            }}
          >
            {t("config.addLlmSource")}
          </Button>
          <FormSectionSubCollapsible
            title={t("config.llmRouterWorkerTitle")}
            defaultOpen={false}
          >
            <Box sx={{ display: "flex", gap: 2, flexWrap: "wrap" }}>
              <TextField
                select
                label={t("config.llmRouterIndex")}
                value={routerIndex === null ? "" : String(routerIndex)}
                onChange={(e) => {
                  setDirty(true);
                  const v = e.target.value;
                  setRouterIndex(
                    v === "" ? null : Math.max(0, parseInt(v, 10) || 0),
                  );
                }}
                size="small"
                sx={{ minWidth: 280, flex: 1 }}
                slotProps={{
                  inputLabel: { shrink: true },
                  select: { native: true },
                }}
                helperText={t("config.llmRouterIndexHelp")}
              >
                <option value="">{t("config.llmIndexNone")}</option>
                {sources.map((s, idx) => (
                  <option key={idx} value={idx}>
                    {idx}: {s.provider.trim() || t("config.llmSource")}
                  </option>
                ))}
              </TextField>
              <TextField
                select
                label={t("config.llmWorkerIndex")}
                value={workerIndex === null ? "" : String(workerIndex)}
                onChange={(e) => {
                  setDirty(true);
                  const v = e.target.value;
                  setWorkerIndex(
                    v === "" ? null : Math.max(0, parseInt(v, 10) || 0),
                  );
                }}
                size="small"
                sx={{ minWidth: 280, flex: 1 }}
                slotProps={{
                  inputLabel: { shrink: true },
                  select: { native: true },
                }}
                helperText={t("config.llmWorkerIndexHelp")}
              >
                <option value="">{t("config.llmIndexNone")}</option>
                {sources.map((s, idx) => (
                  <option key={idx} value={idx}>
                    {idx}: {s.provider.trim() || t("config.llmSource")}
                  </option>
                ))}
              </TextField>
            </Box>
          </FormSectionSubCollapsible>
        </Stack>
        {(saveStatus === "ok" || saveStatus === "fail") && (
          <SaveFeedback
            status={saveStatus}
            message={saveStatus === "ok" ? t("common.saveOk") : saveError}
            autoDismissMs={3000}
            onDismiss={() => {
              setSaveStatus("idle");
              setSaveError("");
            }}
          />
        )}
      </SettingsSection>
    </Box>
  );
}
