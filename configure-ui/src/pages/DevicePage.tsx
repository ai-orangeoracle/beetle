import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import TextField from "@mui/material/TextField";
import Typography from "@mui/material/Typography";
import SaveRounded from "@mui/icons-material/SaveRounded";
import RouterRounded from "@mui/icons-material/RouterRounded";
import ChatRounded from "@mui/icons-material/ChatRounded";
import InfoOutlined from "@mui/icons-material/InfoOutlined";
import RefreshRounded from "@mui/icons-material/RefreshRounded";
import CheckCircleOutlined from "@mui/icons-material/CheckCircleOutlined";
import ErrorOutlined from "@mui/icons-material/ErrorOutlined";
import RemoveCircleOutlined from "@mui/icons-material/RemoveCircleOutlined";
import { InlineAlert, SaveFeedback } from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { BeetleIcon } from "../components/BeetleIcon";
import { useDeviceApi } from "../hooks/useDeviceApi";
import { useDevice } from "../hooks/useDevice";
import {
  type SystemInfoData,
  type ChannelConnectivityItem,
  type HealthData,
} from "../api/endpoints/system";

const DEFAULT_DEVICE_BASE_URL = "http://192.168.4.1";

function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value < 0) return String(value);
  if (value < 1024) return `${value} B`;
  const kb = value / 1024;
  if (kb < 1024) return `${kb.toFixed(1)} KB`;
  const mb = kb / 1024;
  return `${mb.toFixed(1)} MB`;
}

function pressureColor(pressure?: string): string {
  switch (pressure) {
    case "Normal":
      return "var(--semantic-success)";
    case "Cautious":
      return "var(--semantic-warning)";
    case "Critical":
      return "var(--semantic-danger)";
    default:
      return "var(--text-primary)";
  }
}

export function DevicePage() {
  const { t } = useTranslation();
  const { baseUrl, pairingCode, setBaseUrl, setPairingCode } = useDevice();
  const { api, deviceConnected } = useDeviceApi();
  const [urlInput, setUrlInput] = useState(baseUrl || DEFAULT_DEVICE_BASE_URL);
  const [codeInput, setCodeInput] = useState(pairingCode);
  const [probeStatus, setProbeStatus] = useState<
    "idle" | "checking" | "ok" | "fail"
  >("idle");
  const [probeError, setProbeError] = useState("");
  const [systemInfo, setSystemInfo] = useState<SystemInfoData | null>(null);
  const [systemInfoLoading, setSystemInfoLoading] = useState(false);
  const [systemInfoError, setSystemInfoError] = useState("");
  const [channelList, setChannelList] = useState<ChannelConnectivityItem[]>([]);
  const [channelLoading, setChannelLoading] = useState(false);
  const [channelError, setChannelError] = useState("");
  const [healthData, setHealthData] = useState<HealthData | null>(null);
  const [healthLoading, setHealthLoading] = useState(false);
  const [healthError, setHealthError] = useState("");
  const [saveStatus, setSaveStatus] = useState<"idle" | "ok">("idle");

  const handleSave = () => {
    const url = urlInput.trim().replace(/\/$/, "") || DEFAULT_DEVICE_BASE_URL;
    setBaseUrl(url);
    setPairingCode(codeInput.trim());
    setSaveStatus("ok");
  };

  const handleProbe = async () => {
    const url = urlInput.trim().replace(/\/$/, "") || DEFAULT_DEVICE_BASE_URL;
    setProbeStatus("checking");
    setProbeError("");
    const res = await api.device.probe(url);
    if (res.ok) {
      setProbeStatus("ok");
    } else {
      setProbeStatus("fail");
      setProbeError(res.error ?? "");
    }
  };

  useEffect(() => {
    if (!deviceConnected || !baseUrl?.trim()) return;
    let cancelled = false;
    const tid = window.setTimeout(() => {
      if (!cancelled) {
        setSystemInfoLoading(true);
        setSystemInfoError("");
      }
    }, 0);
    api.system
      .info()
      .then((res) => {
        if (cancelled) return;
        setSystemInfoLoading(false);
        if (res.ok && res.data) setSystemInfo(res.data);
        else setSystemInfoError(res.error ?? "");
      })
      .catch(() => {
        if (!cancelled) {
          setSystemInfoLoading(false);
          setSystemInfoError("config.errorNetwork");
        }
      });
    return () => {
      cancelled = true;
      window.clearTimeout(tid);
    };
  }, [api.system, deviceConnected, baseUrl]);

  useEffect(() => {
    if (!deviceConnected || !baseUrl?.trim()) return;
    let cancelled = false;
    const tid = window.setTimeout(() => {
      if (!cancelled) {
        setHealthLoading(true);
        setHealthError("");
      }
    }, 0);
    api.system
      .health()
      .then((res) => {
        if (cancelled) return;
        setHealthLoading(false);
        if (res.ok && res.data) setHealthData(res.data);
        else setHealthError(res.error ?? "");
      })
      .catch(() => {
        if (!cancelled) {
          setHealthLoading(false);
          setHealthError("config.errorNetwork");
        }
      });
    return () => {
      cancelled = true;
      window.clearTimeout(tid);
    };
  }, [api.system, deviceConnected, baseUrl]);

  useEffect(() => {
    if (!deviceConnected || !baseUrl?.trim()) return;
    let cancelled = false;
    const tid = window.setTimeout(() => {
      if (!cancelled) {
        setChannelLoading(true);
        setChannelError("");
      }
    }, 0);
    api.system
      .channelConnectivity()
      .then((res) => {
        if (cancelled) return;
        setChannelLoading(false);
        if (res.ok && res.data?.channels) setChannelList(res.data.channels);
        else setChannelError(res.error ?? "channel connectivity unavailable");
      })
      .catch(() => {
        if (!cancelled) {
          setChannelLoading(false);
          setChannelError("config.errorNetwork");
        }
      });
    return () => {
      cancelled = true;
      window.clearTimeout(tid);
    };
  }, [api.system, deviceConnected, baseUrl]);

  const reloadSystemInfo = () => {
    setSystemInfo(null);
    setSystemInfoError("");
    if (!deviceConnected || !baseUrl?.trim()) return;
    setSystemInfoLoading(true);
    api.system
      .info()
      .then((res) => {
        setSystemInfoLoading(false);
        if (res.ok && res.data) setSystemInfo(res.data);
        else setSystemInfoError(res.error ?? "");
      })
      .catch(() => {
        setSystemInfoLoading(false);
        setSystemInfoError("config.errorNetwork");
      });
  };

  const reloadChannelConnectivity = () => {
    setChannelError("");
    if (!deviceConnected || !baseUrl?.trim()) return;
    setChannelLoading(true);
    api.system
      .channelConnectivity()
      .then((res) => {
        setChannelLoading(false);
        if (res.ok && res.data?.channels) setChannelList(res.data.channels);
        else setChannelError(res.error ?? "channel connectivity unavailable");
      })
      .catch(() => {
        setChannelLoading(false);
        setChannelError("config.errorNetwork");
      });
  };

  const reloadHealth = () => {
    setHealthError("");
    if (!deviceConnected || !baseUrl?.trim()) return;
    setHealthLoading(true);
    api.system
      .health()
      .then((res) => {
        setHealthLoading(false);
        if (res.ok && res.data) setHealthData(res.data);
        else setHealthError(res.error ?? "");
      })
      .catch(() => {
        setHealthLoading(false);
        setHealthError("config.errorNetwork");
      });
  };

  const channelNameKey: Record<string, string> = {
    telegram: "channelTelegram",
    feishu: "channelFeishu",
    dingtalk: "channelDingtalk",
    wecom: "channelWecom",
    qq_channel: "channelQqChannel",
    webhook: "channelWebhook",
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert
        message={
          probeStatus === "fail" ? `${t("device.probeFail")}: ${probeError}` : null
        }
        onRetry={handleProbe}
      />
      <SettingsSection
        icon={<RouterRounded sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("device.sectionConnection")}
        description={t("device.sectionConnectionDesc")}
      >
        <Box sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <TextField
            label={t("device.baseUrlLabel")}
            placeholder={t("device.baseUrlPlaceholder")}
            value={urlInput}
            onChange={(e) => {
              setUrlInput(e.target.value);
              setSaveStatus("idle");
            }}
            size="small"
            fullWidth
            slotProps={{ htmlInput: { style: { fontFamily: "var(--font-mono)" } } }}
          />
          <TextField
            label={t("device.pairingCodeLabel")}
            placeholder={t("device.pairingCodePlaceholder")}
            value={codeInput}
            onChange={(e) => {
              setCodeInput(e.target.value);
              setSaveStatus("idle");
            }}
            size="small"
            fullWidth
            slotProps={{
              htmlInput: { maxLength: 6, style: { fontFamily: "var(--font-mono)" } },
            }}
          />
          <Box sx={{ display: "flex", gap: 1, flexWrap: "wrap" }}>
            <Button
              variant="contained"
              size="small"
              startIcon={<SaveRounded />}
              onClick={handleSave}
              sx={{ borderRadius: "var(--radius-control)" }}
            >
              {t("device.save")}
            </Button>
            <Button
              variant="outlined"
              size="small"
              onClick={handleProbe}
              disabled={probeStatus === "checking"}
              sx={{ borderRadius: "var(--radius-control)" }}
            >
              {probeStatus === "checking"
                ? t("device.probing")
                : t("device.probe")}
            </Button>
          </Box>
          {probeStatus === "ok" && (
            <Typography variant="caption" sx={{ color: "var(--semantic-success)" }}>
              {t("device.probeOk")}
            </Typography>
          )}
          {saveStatus === "ok" && (
            <SaveFeedback
              status="ok"
              message={t("common.saveOk")}
              autoDismissMs={3000}
              onDismiss={() => setSaveStatus("idle")}
            />
          )}
        </Box>
      </SettingsSection>

      {deviceConnected && (
        <Box
          sx={{
            display: "grid",
            gridTemplateColumns: "repeat(3, 1fr)",
            gap: 2,
          }}
        >
          <SettingsSection
            icon={<ChatRounded sx={{ fontSize: "var(--icon-size-md)" }} />}
            label={t("device.sectionChannelConnectivity")}
            accessory={
              <Button
                variant="outlined"
                size="small"
                startIcon={<RefreshRounded />}
                onClick={reloadChannelConnectivity}
                disabled={channelLoading}
                sx={{ borderRadius: "var(--radius-control)" }}
              >
                {t("device.channelRefresh")}
              </Button>
            }
          >
            {channelLoading && (
              <Typography variant="body2" sx={{ color: "var(--muted)" }}>
                {t("common.loading")}
              </Typography>
            )}
            {channelError && !channelList.length && (
              <InlineAlert
                message={
                  channelError === "channel connectivity unavailable"
                    ? t("device.channelConnectivityUnavailable")
                    : channelError
                }
                onRetry={reloadChannelConnectivity}
              />
            )}
            {channelList.length > 0 && (
              <Box sx={{ display: "flex", flexDirection: "column", gap: 1.5 }}>
                {channelList.map((ch) => (
                  <ChannelRow
                    key={ch.id}
                    label={t(`device.${channelNameKey[ch.id] ?? ch.id}`)}
                    configured={ch.configured}
                    ok={ch.ok}
                    message={ch.message}
                    t={t}
                  />
                ))}
                {channelError && (
                  <Typography variant="caption" sx={{ color: "var(--semantic-danger)" }}>
                    {channelError === "channel connectivity unavailable"
                      ? t("device.channelConnectivityUnavailable")
                      : channelError}
                  </Typography>
                )}
                <Typography
                  variant="caption"
                  sx={{ color: "var(--muted)", display: "block", mt: 0.5 }}
                >
                  {t("device.channelConnectivityNote")}
                </Typography>
              </Box>
            )}
          </SettingsSection>

          <SettingsSection
            icon={<BeetleIcon sx={{ width: "var(--icon-size-md)", height: "var(--icon-size-md)" }} />}
            label={t("device.sectionSystemStatus")}
            accessory={
              <Button
                variant="outlined"
                size="small"
                startIcon={<RefreshRounded />}
                onClick={reloadHealth}
                disabled={healthLoading}
                sx={{ borderRadius: "var(--radius-control)" }}
              >
                {t("device.channelRefresh")}
              </Button>
            }
          >
            {healthLoading && (
              <Typography variant="body2" sx={{ color: "var(--muted)" }}>
                {t("common.loading")}
              </Typography>
            )}
            {healthError && !healthData && (
              <InlineAlert
                message={`${t("device.systemStatusLoadFail")}: ${healthError}`}
                onRetry={reloadHealth}
              />
            )}
            {healthData && (
              <Box
                sx={{
                  display: "grid",
                  gridTemplateColumns: "repeat(2, 1fr)",
                  gap: 1.5,
                }}
              >
                {healthData.resource?.pressure && (
                  <Row
                    label={t("device.systemStatusPressure")}
                    value={healthData.resource.pressure}
                    valueColor={pressureColor(healthData.resource.pressure)}
                  />
                )}
                {healthData.resource?.heap_free_internal != null && (
                  <Row
                    label={t("device.systemStatusHeapInternal")}
                    value={formatBytes(healthData.resource.heap_free_internal)}
                  />
                )}
                {healthData.resource?.heap_free_spiram != null && (
                  <Row
                    label={t("device.systemStatusHeapSpiram")}
                    value={formatBytes(healthData.resource.heap_free_spiram)}
                  />
                )}
                {healthData.resource?.heap_largest_block_internal != null && (
                  <Row
                    label={t("device.systemStatusHeapLargest")}
                    value={formatBytes(healthData.resource.heap_largest_block_internal)}
                  />
                )}
                {healthData.resource?.active_http_count != null && (
                  <Row
                    label={t("device.systemStatusActiveHttp")}
                    value={String(healthData.resource.active_http_count)}
                  />
                )}
                {healthData.resource?.inbound_depth != null && (
                  <Row
                    label={t("device.systemStatusInboundDepth")}
                    value={String(healthData.resource.inbound_depth)}
                  />
                )}
                {healthData.resource?.outbound_depth != null && (
                  <Row
                    label={t("device.systemStatusOutboundDepth")}
                    value={String(healthData.resource.outbound_depth)}
                  />
                )}
                {healthData.metrics?.llm_errors != null && (
                  <Row
                    label={t("device.systemStatusLlmErrors")}
                    value={String(healthData.metrics.llm_errors)}
                  />
                )}
                {healthData.metrics?.errors_agent_chat != null && (
                  <Row
                    label={t("device.systemStatusChatErrors")}
                    value={String(healthData.metrics.errors_agent_chat)}
                  />
                )}
                {healthData.metrics?.dispatch_send_fail != null && (
                  <Row
                    label={t("device.systemStatusDispatchFail")}
                    value={String(healthData.metrics.dispatch_send_fail)}
                  />
                )}
              </Box>
            )}
          </SettingsSection>

          <SettingsSection
            icon={<InfoOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
            label={t("device.sectionDeviceInfo")}
          >
            {systemInfoLoading && (
              <Typography variant="body2" sx={{ color: "var(--muted)" }}>
                {t("common.loading")}
              </Typography>
            )}
            {systemInfoError && !systemInfo && (
              <InlineAlert
                message={`${t("device.deviceInfoLoadFail")}: ${systemInfoError}`}
                onRetry={reloadSystemInfo}
              />
            )}
            {systemInfo && (
              <Box
                sx={{
                  display: "grid",
                  gridTemplateColumns: "repeat(2, 1fr)",
                  gap: 1.5,
                }}
              >
                <Row label={t("device.deviceInfoProduct")} value={systemInfo.product_name} />
                <Row label={t("device.deviceInfoFirmware")} value={systemInfo.firmware_version} />
                <Row label={t("device.deviceInfoStatus")} value={systemInfo.system_status} />
                {systemInfo.board_id && (
                  <Row label={t("device.deviceInfoBoardId")} value={systemInfo.board_id} />
                )}
                {systemInfo.locale != null && (
                  <Row label={t("device.deviceInfoLocale")} value={systemInfo.locale} />
                )}
                {systemInfo.ota_available != null && (
                  <Row
                    label={t("device.deviceInfoOtaAvailable")}
                    value={systemInfo.ota_available ? t("common.yes") : t("common.no")}
                  />
                )}
                {systemInfo.current_time && (
                  <Row label={t("device.deviceInfoCurrentTime")} value={systemInfo.current_time} />
                )}
              </Box>
            )}
          </SettingsSection>
        </Box>
      )}
    </Box>
  );
}

function Row({
  label,
  value,
  valueColor = "var(--text-primary)",
}: {
  label: string;
  value: string;
  valueColor?: string;
}) {
  return (
    <Box sx={{ display: "flex", flexWrap: "wrap", gap: 0.5, alignItems: "baseline" }}>
      <Typography variant="body2" sx={{ color: "var(--muted)", minWidth: 100 }}>
        {label}:
      </Typography>
      <Typography variant="body2" sx={{ fontFamily: "var(--font-mono)", color: valueColor }}>
        {value}
      </Typography>
    </Box>
  );
}

function ChannelRow({
  label,
  configured,
  ok,
  message,
  t,
}: {
  label: string;
  configured: boolean;
  ok: boolean;
  message?: string | null;
  t: (key: string) => string;
}) {
  const statusText = configured
    ? ok
      ? t("device.channelOk")
      : message ?? t("device.channelFail")
    : t("device.channelNotConfigured");
  const statusColor = configured
    ? ok
      ? "var(--semantic-success)"
      : "var(--semantic-danger)"
    : "var(--muted)";
  const StatusIcon = configured ? (ok ? CheckCircleOutlined : ErrorOutlined) : RemoveCircleOutlined;
  return (
    <Box sx={{ display: "flex", alignItems: "center", gap: 1 }}>
      <Typography variant="body2" sx={{ color: "var(--muted)", minWidth: 80 }}>
        {label}:
      </Typography>
      <Box sx={{ display: "flex", alignItems: "center", gap: 0.5, marginLeft: "auto", justifyContent: "flex-end" }}>
        <StatusIcon sx={{ fontSize: "var(--icon-size-sm)", color: statusColor }} aria-hidden />
        <Typography variant="body2" sx={{ color: statusColor }}>
          {statusText}
        </Typography>
      </Box>
    </Box>
  );
}
