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
import { createAsyncState } from "../types/asyncState";

export function SystemLogsPage() {
  const { t } = useTranslation();
  const { api, ready } = useDeviceApi();
  const [logsState, setLogsState] = useState(
    createAsyncState<{ health: HealthData | null; diagnose: DiagnoseItem[] }>({
      health: null,
      diagnose: [],
    }),
  );

  const loadLogs = useCallback(() => {
    if (!ready) return;
    setLogsState((prev) => ({ ...prev, loading: true, error: "" }));
    Promise.all([api.system.health(), api.system.diagnose()])
      .then(([healthRes, diagnoseRes]) => {
        const nextHealth = healthRes.ok && healthRes.data ? healthRes.data : null;
        const nextDiagnose = diagnoseRes.ok && diagnoseRes.data ? diagnoseRes.data : [];
        const nextError =
          !healthRes.ok ? (healthRes.error ?? "") : !diagnoseRes.ok ? (diagnoseRes.error ?? "") : "";
        setLogsState({ loading: false, error: nextError, data: { health: nextHealth, diagnose: nextDiagnose } });
      })
      .catch(() =>
        setLogsState((prev) => ({ ...prev, loading: false, error: "config.errorNetwork" })),
      );
  }, [api.system, ready]);

  useEffect(() => {
    if (!ready) {
      queueMicrotask(() => {
        setLogsState(createAsyncState({ health: null, diagnose: [] }));
      });
      return;
    }
    // 避免在 effect 主体中同步触发 setState（触发 react-hooks 规则）
    queueMicrotask(() => {
      loadLogs();
    });
  }, [ready, loadLogs]);

  const severityColor = (s: string) => {
    if (s === "ok") return "var(--semantic-success)";
    if (s === "warn") return "var(--semantic-warning)";
    return "var(--semantic-danger)";
  };

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={logsState.error || null} onRetry={loadLogs} />
      
      <SettingsSection
        icon={<DescriptionOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("systemLogs.sectionLogs")}
      >
        {!ready ? (
          <Typography variant="body2" color="text.secondary">
            {t("device.pageDesc")}
          </Typography>
        ) : logsState.loading ? (
          <SectionLoadingSkeleton />
        ) : (
          <Box sx={{ display: "flex", flexDirection: "column", gap: 2 }}>
            {logsState.data.health && (
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
                      primary={`wifi: ${logsState.data.health.wifi ?? "—"}`}
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
                      primary={`inbound_depth: ${logsState.data.health.inbound_depth ?? "—"}`}
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
                      primary={`outbound_depth: ${logsState.data.health.outbound_depth ?? "—"}`}
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
                      primary={`last_error: ${logsState.data.health.last_error ?? "none"}`}
                      slotProps={{
                        primary: {
                          variant: "body2",
                          sx: { fontFamily: "var(--font-mono)" },
                        },
                      }}
                    />
                  </ListItem>
                  {logsState.data.health.resource && (
                    <>
                      <ListItem sx={{ py: 0.5, px: 0 }}>
                        <ListItemText
                          primary={`resource.pressure: ${logsState.data.health.resource.pressure ?? "—"}`}
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
                          primary={`resource.heap_largest_block_internal: ${logsState.data.health.resource.heap_largest_block_internal ?? "—"}`}
                          slotProps={{
                            primary: {
                              variant: "body2",
                              sx: { fontFamily: "var(--font-mono)" },
                            },
                          }}
                        />
                      </ListItem>
                    </>
                  )}
                </List>
              </Box>
            )}
            {logsState.data.diagnose.length > 0 && (
              <Box>
                <Typography
                  variant="caption"
                  sx={{ fontWeight: 700, color: "var(--muted)" }}
                >
                  GET /api/diagnose
                </Typography>
                <List dense disablePadding>
                  {logsState.data.diagnose.map((item, i) => (
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
            {!logsState.data.health && logsState.data.diagnose.length === 0 && !logsState.loading && ready && (
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
