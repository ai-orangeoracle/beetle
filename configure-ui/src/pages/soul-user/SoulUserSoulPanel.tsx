import { useTranslation } from "react-i18next";
import Box from "@mui/material/Box";
import Button from "@mui/material/Button";
import SaveRounded from "@mui/icons-material/SaveRounded";
import PsychologyOutlined from "@mui/icons-material/PsychologyOutlined";
import {
  InlineAlert,
  SaveFeedback,
  SectionLoadingSkeleton,
} from "../../components/form";
import { SettingsSection } from "../../components/SettingsSection";
import { useSoulUserConfig } from "../../contexts/SoulUserConfigContext";
import { SoulFormBody } from "./formBodies";

/** 个性配置 → SOUL Tab（路由子页） */
export function SoulUserSoulPanel() {
  const { t } = useTranslation();
  const {
    ready,
    loadError,
    retryLoad,
    soulForm,
    setSoulForm,
    soulState,
    soulSaveStatus,
    soulError,
    handleSaveSoul,
    dismissSoulSaveFeedback,
  } = useSoulUserConfig();

  const soulAlert = loadError || soulState.error || null;
  const saveDisabled =
    !ready || soulSaveStatus === "saving" || soulState.loading;

  return (
    <Box sx={{ display: "flex", flexDirection: "column", gap: 4 }}>
      <InlineAlert message={soulAlert} onRetry={retryLoad} />
      <SettingsSection
        icon={<PsychologyOutlined sx={{ fontSize: "var(--icon-size-md)" }} />}
        label={t("soulUser.sectionSoul")}
        description={t("soulUser.soulDesc")}
        accessory={
          <Button
            size="small"
            variant="contained"
            startIcon={<SaveRounded />}
            onClick={() => {
              void handleSaveSoul();
            }}
            disabled={saveDisabled}
          >
            {soulSaveStatus === "saving" ? t("common.saving") : t("common.save")}
          </Button>
        }
        belowTitleRow={
          soulSaveStatus === "ok" || soulSaveStatus === "fail" ? (
            <SaveFeedback
              placement="belowTitle"
              status={soulSaveStatus}
              message={soulSaveStatus === "ok" ? t("common.saveOk") : soulError}
              autoDismissMs={3000}
              onDismiss={dismissSoulSaveFeedback}
            />
          ) : null
        }
      >
        {soulState.loading ? (
          <SectionLoadingSkeleton />
        ) : (
          <SoulFormBody form={soulForm} setForm={setSoulForm} t={t} />
        )}
      </SettingsSection>
    </Box>
  );
}
