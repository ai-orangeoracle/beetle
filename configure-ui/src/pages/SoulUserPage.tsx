import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import TextField from "@mui/material/TextField";
import PsychologyOutlined from "@mui/icons-material/PsychologyOutlined";
import PersonOutlined from "@mui/icons-material/PersonOutlined";
import SaveRounded from "@mui/icons-material/SaveRounded";
import {
  InlineAlert,
  SaveFeedback,
  SectionLoadingSkeleton,
} from "../components/form";
import { SettingsSection } from "../components/SettingsSection";
import { useDeviceApi, API_ERROR } from "../hooks/useDeviceApi";
import { useUnsaved } from "../hooks/useUnsaved";
import { createAsyncState } from "../types/asyncState";

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
  const [soulState, setSoulState] = useState(createAsyncState(""));
  const [userState, setUserState] = useState(createAsyncState(""));
  const [savedSoul, setSavedSoul] = useState("");
  const [savedUser, setSavedUser] = useState("");
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
    setSoulState((prev) => ({ ...prev, loading: true }));
    setLoadError("");
    api.soul
      .get()
      .then((res) => {
        if (res.ok) {
          const data = res.data ?? "";
          setSoulState({ data, loading: false, error: "" });
          setSavedSoul(data);
        } else {
          setSoulState((prev) => ({ ...prev, loading: false, error: res.error ?? "" }));
          setLoadError(res.error ?? "");
        }
      })
      .catch(() => {
        setSoulState((prev) => ({ ...prev, loading: false, error: "config.errorNetwork" }));
        setLoadError("config.errorNetwork");
      });
  }, [api.soul, ready]);

  const loadUser = useCallback(() => {
    if (!ready) return;
    setUserState((prev) => ({ ...prev, loading: true }));
    api.user
      .get()
      .then((res) => {
        if (res.ok) {
          const data = res.data ?? "";
          setUserState({ data, loading: false, error: "" });
          setSavedUser(data);
        } else {
          setUserState((prev) => ({ ...prev, loading: false, error: res.error ?? "" }));
        }
      })
      .catch(() =>
        setUserState((prev) => ({ ...prev, loading: false, error: "config.errorNetwork" })),
      );
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
    if (soulState.data.length > MAX_CONTENT) {
      setSoulSaveStatus("fail");
      setSoulError(t("config.validation.contentTooLong"));
      return;
    }
    setSoulSaveStatus("saving");
    setSoulError("");
    const res = await api.soul.save(soulState.data);
    setSoulSaveStatus(res.ok ? "ok" : "fail");
    setSoulError(apiErrorMessage(res.error, t));
    if (res.ok) setSavedSoul(soulState.data);
  };

  const handleSaveUser = async () => {
    if (userState.data.length > MAX_CONTENT) {
      setUserSaveStatus("fail");
      setUserError(t("config.validation.contentTooLong"));
      return;
    }
    setUserSaveStatus("saving");
    setUserError("");
    const res = await api.user.save(userState.data);
    setUserSaveStatus(res.ok ? "ok" : "fail");
    setUserError(apiErrorMessage(res.error, t));
    if (res.ok) setSavedUser(userState.data);
  };

  useEffect(() => {
    setDirty(soulState.data !== savedSoul || userState.data !== savedUser);
  }, [soulState.data, savedSoul, userState.data, savedUser, setDirty]);

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
        belowTitleRow={
          soulSaveStatus === "ok" || soulSaveStatus === "fail" ? (
            <SaveFeedback
              placement="belowTitle"
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
          ) : null
        }
      >
        {soulState.loading ? (
          <SectionLoadingSkeleton />
        ) : (
          <>
            <TextField
              placeholder={t("soulUser.soulPlaceholder")}
              value={soulState.data}
              onChange={(e) => setSoulState((prev) => ({ ...prev, data: e.target.value }))}
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
        belowTitleRow={
          userSaveStatus === "ok" || userSaveStatus === "fail" ? (
            <SaveFeedback
              placement="belowTitle"
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
          ) : null
        }
      >
        {userState.loading ? (
          <SectionLoadingSkeleton />
        ) : (
          <>
            <TextField
              placeholder={t("soulUser.userPlaceholder")}
              value={userState.data}
              onChange={(e) => setUserState((prev) => ({ ...prev, data: e.target.value }))}
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
          </>
        )}
      </SettingsSection>
    </Box>
  );
}
