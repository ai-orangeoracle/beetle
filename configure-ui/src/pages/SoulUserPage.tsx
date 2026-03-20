import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import TextField from "@mui/material/TextField";
import PsychologyOutlined from "@mui/icons-material/PsychologyOutlined";
import PersonOutlined from "@mui/icons-material/PersonOutlined";
import SaveRounded from "@mui/icons-material/SaveRounded";
import { InlineAlert, SaveFeedback, SectionLoadingSkeleton } from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { useDeviceApi, API_ERROR } from "../hooks/useDeviceApi";
import { useUnsaved } from "../hooks/useUnsaved";

const MAX_CONTENT = 32 * 1024;

function apiErrorMessage(
  error: string | undefined,
  t: (k: string) => string,
): string {
  if (error === API_ERROR.PAIRING_REQUIRED)
    return t("device.pairingCodeRequired");
  return error ?? "";
}

export function SoulUserPage() {
  const { t } = useTranslation();
  const { api, ready } = useDeviceApi();
  const { setDirty } = useUnsaved();
  const [soul, setSoul] = useState("");
  const [user, setUser] = useState("");
  const [savedSoul, setSavedSoul] = useState("");
  const [savedUser, setSavedUser] = useState("");
  const [soulLoading, setSoulLoading] = useState(false);
  const [userLoading, setUserLoading] = useState(false);
  const [soulSaveStatus, setSoulSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [userSaveStatus, setUserSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [soulError, setSoulError] = useState("");
  const [userError, setUserError] = useState("");
  const [loadError, setLoadError] = useState("");

  const loadSoul = useCallback(() => {
    if (!ready) return;
    setSoulLoading(true);
    setLoadError("");
    api.soul
      .get()
      .then((res) => {
        if (res.ok) {
          const data = res.data ?? "";
          setSoul(data);
          setSavedSoul(data);
        } else setLoadError(res.error ?? "");
      })
      .catch(() => setLoadError("config.errorNetwork"))
      .finally(() => setSoulLoading(false));
  }, [api.soul, ready]);

  const loadUser = useCallback(() => {
    if (!ready) return;
    setUserLoading(true);
    api.user
      .get()
      .then((res) => {
        if (res.ok) {
          const data = res.data ?? "";
          setUser(data);
          setSavedUser(data);
        }
      })
      .finally(() => setUserLoading(false));
  }, [api.user, ready]);

  const retryLoad = useCallback(() => {
    loadSoul();
    loadUser();
  }, [loadSoul, loadUser]);

  useEffect(() => {
    if (!ready) return;
    // 避免在 effect 主体中同步触发 setState（触发 react-hooks 规则）
    queueMicrotask(() => {
      loadSoul();
    });
  }, [ready, loadSoul]);

  useEffect(() => {
    if (!ready) return;
    // 避免在 effect 主体中同步触发 setState（触发 react-hooks 规则）
    queueMicrotask(() => {
      loadUser();
    });
  }, [ready, loadUser]);

  const handleSaveSoul = async () => {
    if (soul.length > MAX_CONTENT) {
      setSoulSaveStatus("fail");
      setSoulError(t("config.validation.contentTooLong"));
      return;
    }
    setSoulSaveStatus("saving");
    setSoulError("");
    const res = await api.soul.save(soul);
    setSoulSaveStatus(res.ok ? "ok" : "fail");
    setSoulError(apiErrorMessage(res.error, t));
    if (res.ok) setSavedSoul(soul);
  };

  const handleSaveUser = async () => {
    if (user.length > MAX_CONTENT) {
      setUserSaveStatus("fail");
      setUserError(t("config.validation.contentTooLong"));
      return;
    }
    setUserSaveStatus("saving");
    setUserError("");
    const res = await api.user.save(user);
    setUserSaveStatus(res.ok ? "ok" : "fail");
    setUserError(apiErrorMessage(res.error, t));
    if (res.ok) setSavedUser(user);
  };

  useEffect(() => {
    setDirty(soul !== savedSoul || user !== savedUser);
  }, [soul, savedSoul, user, savedUser, setDirty]);

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={loadError || null} onRetry={retryLoad} />
      <SettingsSection
        icon={<PsychologyOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("soulUser.sectionSoul")}
        description={t("soulUser.soulDesc")}
        accessory={
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveRounded />}
            onClick={handleSaveSoul}
            disabled={!ready || soulSaveStatus === "saving"}
            sx={{ borderRadius: "var(--radius-control)" }}
          >
            {soulSaveStatus === "saving"
              ? t("common.saving")
              : t("common.save")}
          </Button>
        }
      >
        {soulLoading ? (
          <SectionLoadingSkeleton />
        ) : (
          <>
            <TextField
              placeholder={t("soulUser.soulPlaceholder")}
              value={soul}
              onChange={(e) => setSoul(e.target.value)}
              multiline
              minRows={6}
              maxRows={16}
              fullWidth
              size="small"
              sx={{
                "& .MuiInputBase-root": { alignItems: "flex-start" },
                "& textarea": {
                  fontFamily: "inherit",
                  fontSize: "var(--font-size-body)",
                },
              }}
            />
            {(soulSaveStatus === "ok" || soulSaveStatus === "fail") && (
              <SaveFeedback
                status={soulSaveStatus}
                message={
                  soulSaveStatus === "ok" ? t("common.saveOk") : soulError
                }
                autoDismissMs={3000}
                onDismiss={() => {
                  setSoulSaveStatus("idle");
                  setSoulError("");
                }}
              />
            )}
          </>
        )}
      </SettingsSection>

      <SettingsSection
        icon={<PersonOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("soulUser.sectionUser")}
        description={t("soulUser.userDesc")}
        accessory={
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveRounded />}
            onClick={handleSaveUser}
            disabled={!ready || userSaveStatus === "saving"}
            sx={{ borderRadius: "var(--radius-control)" }}
          >
            {userSaveStatus === "saving"
              ? t("common.saving")
              : t("common.save")}
          </Button>
        }
      >
        {userLoading ? (
          <SectionLoadingSkeleton />
        ) : (
          <>
            <TextField
              placeholder={t("soulUser.userPlaceholder")}
              value={user}
              onChange={(e) => setUser(e.target.value)}
              multiline
              minRows={4}
              maxRows={12}
              fullWidth
              size="small"
              sx={{
                "& .MuiInputBase-root": { alignItems: "flex-start" },
                "& textarea": {
                  fontFamily: "inherit",
                  fontSize: "var(--font-size-body)",
                },
              }}
            />
            {(userSaveStatus === "ok" || userSaveStatus === "fail") && (
              <SaveFeedback
                status={userSaveStatus}
                message={
                  userSaveStatus === "ok" ? t("common.saveOk") : userError
                }
                autoDismissMs={3000}
                onDismiss={() => {
                  setUserSaveStatus("idle");
                  setUserError("");
                }}
              />
            )}
          </>
        )}
      </SettingsSection>
    </Box>
  );
}
