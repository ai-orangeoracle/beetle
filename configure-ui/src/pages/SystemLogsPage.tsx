import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import List from "@mui/material/List";
import ListItem from "@mui/material/ListItem";
import ListItemText from "@mui/material/ListItemText";
import Typography from "@mui/material/Typography";
import DescriptionOutlined from "@mui/icons-material/DescriptionOutlined";
import { InlineAlert, SectionLoadingSkeleton } from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { useDeviceApi } from "../hooks/useDeviceApi";
import type { HealthData, DiagnoseItem } from "../api/endpoints/system";

export function SystemLogsPage() {
  const { t } = useTranslation();
  const { api, ready } = useDeviceApi();
  const [health, setHealth] = useState<HealthData | null>(null);
  const [diagnose, setDiagnose] = useState<DiagnoseItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const loadLogs = useCallback(() => {
    if (!ready) return;
    setLoading(true);
    setError("");
    Promise.all([api.system.health(), api.system.diagnose()])
      .then(([healthRes, diagnoseRes]) => {
        if (healthRes.ok && healthRes.data) setHealth(healthRes.data);
        else {
          setHealth(null);
          if (!healthRes.ok) setError(healthRes.error ?? "");
        }
        if (diagnoseRes.ok && diagnoseRes.data) setDiagnose(diagnoseRes.data);
        else {
          setDiagnose([]);
          if (!diagnoseRes.ok && !healthRes.ok)
            setError(diagnoseRes.error ?? "");
        }
      })
      .catch(() => setError("config.errorNetwork"))
      .finally(() => setLoading(false));
  }, [api.system, ready]);

  useEffect(() => {
    if (!ready) {
      queueMicrotask(() => {
        setHealth(null);
        setDiagnose([]);
        setError("");
      });
      return;
    }
    loadLogs();
  }, [ready, loadLogs]);

  const severityColor = (s: string) => {
    if (s === "ok") return "var(--primary)";
    if (s === "warn") return "var(--warning)";
    return "var(--error, #b71c1c)";
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={error || null} onRetry={loadLogs} />
      <SettingsSection
        icon={<DescriptionOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("systemLogs.sectionLogs")}
      >
        {!ready ? (
          <Typography variant="body2" color="text.secondary">
            {t("device.pageDesc")}
          </Typography>
        ) : loading ? (
          <SectionLoadingSkeleton />
        ) : (
          <Box sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {health && (
              <Box>
                <Typography
                  variant="caption"
                  sx={{ fontWeight: 700, color: "var(--muted)" }}
                >
                  GET /api/health
                </Typography>
                <List dense disablePadding>
                  <ListItem sx={{ py: 0.5, px: 0 }}>
                    <ListItemText
                      primary={`wifi: ${health.wifi ?? "—"}`}
                      slotProps={{
                        primary: {
                          variant: "body2",
                          sx: { fontFamily: "var(--font-mono)" },
                        },
                      }}
                    />
                  </ListItem>
                  <ListItem sx={{ py: 0.5, px: 0 }}>
                    <ListItemText
                      primary={`inbound_depth: ${health.inbound_depth ?? "—"}`}
                      slotProps={{
                        primary: {
                          variant: "body2",
                          sx: { fontFamily: "var(--font-mono)" },
                        },
                      }}
                    />
                  </ListItem>
                  <ListItem sx={{ py: 0.5, px: 0 }}>
                    <ListItemText
                      primary={`outbound_depth: ${health.outbound_depth ?? "—"}`}
                      slotProps={{
                        primary: {
                          variant: "body2",
                          sx: { fontFamily: "var(--font-mono)" },
                        },
                      }}
                    />
                  </ListItem>
                  <ListItem sx={{ py: 0.5, px: 0 }}>
                    <ListItemText
                      primary={`last_error: ${health.last_error ?? "none"}`}
                      slotProps={{
                        primary: {
                          variant: "body2",
                          sx: { fontFamily: "var(--font-mono)" },
                        },
                      }}
                    />
                  </ListItem>
                </List>
              </Box>
            )}
            {diagnose.length > 0 && (
              <Box>
                <Typography
                  variant="caption"
                  sx={{ fontWeight: 700, color: "var(--muted)" }}
                >
                  GET /api/diagnose
                </Typography>
                <List dense disablePadding>
                  {diagnose.map((item, i) => (
                    <ListItem
                      key={i}
                      sx={{ py: 0.5, px: 0, alignItems: "flex-start" }}
                    >
                      <Box
                        sx={{
                          width: 8,
                          height: 8,
                          borderRadius: "50%",
                          bgcolor: severityColor(item.severity),
                          mt: 1.2,
                          mr: 1,
                          flexShrink: 0,
                        }}
                      />
                      <ListItemText
                        primary={`[${item.severity}] ${item.category}: ${item.message}`}
                        slotProps={{
                          primary: {
                            variant: "body2",
                            sx: {
                              fontFamily: "var(--font-mono)",
                              fontSize: "0.8125rem",
                            },
                          },
                        }}
                      />
                    </ListItem>
                  ))}
                </List>
              </Box>
            )}
            {!health && diagnose.length === 0 && !loading && ready && (
              <Typography variant="body2" color="text.secondary">
                {t("systemLogs.emptyLogs")}
              </Typography>
            )}
          </Box>
        )}
      </SettingsSection>
    </Box>
  );
}
