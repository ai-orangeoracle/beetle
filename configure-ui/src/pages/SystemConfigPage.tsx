import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import Slider from "@mui/material/Slider";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import SettingsEthernetOutlined from "@mui/icons-material/SettingsEthernetOutlined";
import SaveRounded from "@mui/icons-material/SaveRounded";
import WifiFind from "@mui/icons-material/WifiFind";
import {
  FormLoadingSkeleton,
  FormSectionSub,
  InlineAlert,
  SaveFeedback,
} from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { useConfig } from "../hooks/useConfig";
import { useDevice } from "../hooks/useDevice";
import { useRevealedPassword } from "../hooks/useRevealedPassword";
import { useToast } from "../hooks/useToast";
import { useUnsaved } from "../hooks/useUnsaved";
import { getWifiScan, type WifiApEntry } from "../api/endpoints/system";
import type { AppConfig } from "../types/appConfig";

const WIFI_MANUAL = "__manual__";

const MAX_LEN = 64;
const SESSION_MIN = 1;
const SESSION_MAX = 128;

/** 简单校验：非空时须含 :// 且 scheme 后为非空（后端会做完整校验）。 */
function isValidProxyUrl(v: string): boolean {
  const s = v.trim();
  if (!s) return true;
  const i = s.indexOf("://");
  return i !== -1 && i + 3 < s.length;
}

function validateSystem(
  form: AppConfig,
  t: (k: string) => string,
): string | null {
  if (!isValidProxyUrl(form.proxy_url ?? ""))
    return t("config.validation.proxyUrlInvalid");
  const wifiPassSet = !!form.wifi_pass.trim();
  if (wifiPassSet && !form.wifi_ssid.trim())
    return t("config.validation.wifiSsidRequired");
  const n = form.session_max_messages;
  if (n < SESSION_MIN || n > SESSION_MAX)
    return t("config.validation.sessionMaxMessages");
  return null;
}

export function SystemConfigPage() {
  const { t } = useTranslation();
  const { baseUrl } = useDevice();
  const { config, loadConfig, saveSystem, loading, error } = useConfig();
  const { setDirty } = useUnsaved();
  const [form, setForm] = useState<AppConfig | null>(null);
  const [wifiScanList, setWifiScanList] = useState<WifiApEntry[] | null>(null);
  const [wifiScanLoading, setWifiScanLoading] = useState(false);
  const [saveStatus, setSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [saveError, setSaveError] = useState("");
  const loadAttemptedRef = useRef(false);
  const { showToast } = useToast();
  const { type: wifiPassType, inputProps: wifiPassInputProps } =
    useRevealedPassword();

  const handleWifiScan = async () => {
    if (!baseUrl?.trim()) return;
    setWifiScanLoading(true);
    setWifiScanList(null);
    const res = await getWifiScan(baseUrl);
    setWifiScanLoading(false);
    if (res.ok && Array.isArray(res.data)) {
      setWifiScanList(res.data);
    } else {
      setWifiScanList([]);
      showToast(res.error ?? t("config.wifiScanFailed"), { variant: "error" });
    }
  };

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
      queueMicrotask(() => setForm(null));
      return;
    }
    queueMicrotask(() => setForm(config));
  }, [config]);

  const update = (key: keyof AppConfig, value: string | number) => {
    setDirty(true);
    setForm((prev) => (prev ? { ...prev, [key]: value } : null));
  };

  const handleSave = async () => {
    if (!config || !form) return;
    const err = validateSystem(form, t);
    if (err) {
      setSaveStatus("fail");
      setSaveError(err);
      return;
    }
    const segment = {
      wifi_ssid: form.wifi_ssid,
      wifi_pass: form.wifi_pass,
      proxy_url: form.proxy_url ?? "",
      session_max_messages: form.session_max_messages,
      tg_group_activation: form.tg_group_activation,
    };
    setSaveStatus("saving");
    setSaveError("");
    const result = await saveSystem(segment);
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
          icon={
            <SettingsEthernetOutlined
              sx={{ fontSize: "var(--icon-size-md)" }}
            />
          }
          label={t("config.sectionSystem")}
        >
          <FormLoadingSkeleton />
        </SettingsSection>
      </Box>
    );
  }

  const saveDisabled = saveStatus === "saving" || !form;
  const proxyUrlError =
    form && !isValidProxyUrl(form.proxy_url ?? "")
      ? t("config.validation.proxyUrlInvalid")
      : "";
  const sessionError =
    form &&
    (form.session_max_messages < SESSION_MIN ||
      form.session_max_messages > SESSION_MAX)
      ? t("config.validation.sessionMaxMessages")
      : "";

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={error} onRetry={loadConfig} />
      <SettingsSection
        icon={
          <SettingsEthernetOutlined sx={{ fontSize: "var(--icon-size-md)" }} />
        }
        label={t("config.sectionSystem")}
        description={t("config.sectionSystemDesc")}
        accessory={
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveRounded />}
            onClick={handleSave}
            disabled={saveDisabled}
            title={!form ? t("config.hintSaveNeedDevice") : undefined}
            sx={{ borderRadius: "var(--radius-control)" }}
          >
            {saveStatus === "saving" ? t("common.saving") : t("common.save")}
          </Button>
        }
      >
        {!form ? (
          <Typography variant="body2" color="text.secondary" sx={{ py: 2 }}>
            {t("config.hintSaveNeedDevice")}
          </Typography>
        ) : (
          <>
        <FormSectionSub title={t("config.wifi")}>
          <Box sx={{ display: "flex", gap: 1, alignItems: "flex-start", flexWrap: "wrap" }}>
            <Button
              variant="outlined"
              size="small"
              startIcon={<WifiFind sx={{ fontSize: "var(--icon-size-sm)" }} />}
              onClick={handleWifiScan}
              disabled={!baseUrl?.trim() || wifiScanLoading}
            >
              {wifiScanLoading ? t("config.wifiScanning") : t("config.wifiScan")}
            </Button>
          </Box>
          {wifiScanList && wifiScanList.length > 0 ? (
            <>
              <TextField
                select
                label={t("config.wifiSsid")}
                value={
                  wifiScanList.some((ap) => ap.ssid === form.wifi_ssid)
                    ? form.wifi_ssid
                    : WIFI_MANUAL
                }
                onChange={(e) => {
                  const v = e.target.value;
                  if (v !== WIFI_MANUAL) update("wifi_ssid", v);
                }}
                size="small"
                fullWidth
                slotProps={{
                  inputLabel: { shrink: true },
                  select: { native: true },
                }}
              >
                {wifiScanList.map((ap) => (
                  <option key={ap.ssid} value={ap.ssid}>
                    {ap.ssid} ({ap.rssi} dBm)
                  </option>
                ))}
                <option value={WIFI_MANUAL}>{t("config.wifiSsidManual")}</option>
              </TextField>
              {(form.wifi_ssid === "" ||
                !wifiScanList.some((ap) => ap.ssid === form.wifi_ssid)) && (
                <TextField
                  label={t("config.wifiSsidManual")}
                  value={form.wifi_ssid}
                  onChange={(e) => update("wifi_ssid", e.target.value)}
                  size="small"
                  fullWidth
                  placeholder={t("config.wifiSsidHelp")}
                  slotProps={{ htmlInput: { maxLength: MAX_LEN } }}
                />
              )}
            </>
          ) : (
            <TextField
              label={t("config.wifiSsid")}
              value={form.wifi_ssid}
              onChange={(e) => update("wifi_ssid", e.target.value)}
              size="small"
              fullWidth
              helperText={t("config.wifiSsidHelp")}
              slotProps={{ htmlInput: { maxLength: MAX_LEN } }}
            />
          )}
          <TextField
            label={t("config.wifiPass")}
            value={form.wifi_pass}
            onChange={(e) => update("wifi_pass", e.target.value)}
            type={wifiPassType}
            size="small"
            fullWidth
            slotProps={{
              htmlInput: {
                maxLength: MAX_LEN,
                style: { fontFamily: "var(--font-mono)" },
                ...wifiPassInputProps,
              },
            }}
          />
        </FormSectionSub>

        <FormSectionSub title={t("config.proxy")}>
          <TextField
            label={t("config.proxyUrl")}
            value={form.proxy_url ?? ""}
            onChange={(e) => update("proxy_url", e.target.value)}
            placeholder={t("config.placeholderProxyUrl")}
            size="small"
            fullWidth
            error={!!proxyUrlError}
            helperText={proxyUrlError || t("config.proxyUrlHint")}
            slotProps={{
              htmlInput: {
                maxLength: 256,
                style: { fontFamily: "var(--font-mono)" },
              },
            }}
          />
        </FormSectionSub>

        <FormSectionSub title={t("config.session")}>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
            {t("config.sessionMaxMessages")}: {form.session_max_messages}
          </Typography>
          <Slider
            value={form.session_max_messages}
            min={SESSION_MIN}
            max={SESSION_MAX}
            valueLabelDisplay="auto"
            onChange={(_, value) =>
              update(
                "session_max_messages",
                Array.isArray(value) ? value[0] : value,
              )
            }
            sx={{
              maxWidth: 320,
              mt: 0.5,
              "& .MuiSlider-thumb": { borderRadius: "var(--radius-control)" },
              "& .MuiSlider-track": { borderRadius: 1 },
              "& .MuiSlider-rail": { borderRadius: 1 },
            }}
          />
          {(sessionError || t("config.sessionMaxMessagesHelp")) && (
            <Typography
              variant="caption"
              sx={{
                display: "block",
                mt: 0.5,
                color: sessionError ? "var(--rating-low)" : "var(--muted)",
              }}
            >
              {sessionError || t("config.sessionMaxMessagesHelp")}
            </Typography>
          )}
        </FormSectionSub>

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
          </>
        )}
      </SettingsSection>
    </Box>
  );
}
