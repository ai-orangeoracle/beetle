import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type Dispatch,
  type ReactNode,
  type SetStateAction,
} from "react";
import { useTranslation } from "react-i18next";
import { useDeviceApi, API_ERROR } from "../hooks/useDeviceApi";
import { useUnsaved } from "../hooks/useUnsaved";
import { createAsyncState, type AsyncState } from "../types/asyncState";
import {
  defaultSoulForm,
  defaultUserForm,
  parseSoul,
  parseUser,
  serializeSoul,
  serializeUser,
  type SoulFormState,
  type UserFormState,
} from "../util/soulUserFormat";

const MAX_CONTENT = 32 * 1024;

function apiErrorMessage(
  error: string | undefined,
  t: (k: string) => string,
): string {
  if (error === API_ERROR.PAIRING_REQUIRED)
    return t("device.pairingCodeRequired");
  return error ?? "";
}

export type SoulUserConfigContextValue = {
  ready: boolean;
  loadError: string;
  retryLoad: () => void;
  soulForm: SoulFormState;
  setSoulForm: Dispatch<SetStateAction<SoulFormState>>;
  userForm: UserFormState;
  setUserForm: Dispatch<SetStateAction<UserFormState>>;
  soulState: AsyncState<string>;
  userState: AsyncState<string>;
  soulSaveStatus: "idle" | "saving" | "ok" | "fail";
  userSaveStatus: "idle" | "saving" | "ok" | "fail";
  soulError: string;
  userError: string;
  handleSaveSoul: () => Promise<void>;
  handleSaveUser: () => Promise<void>;
  dismissSoulSaveFeedback: () => void;
  dismissUserSaveFeedback: () => void;
};

const SoulUserConfigContext = createContext<SoulUserConfigContextValue | null>(
  null,
);

export function SoulUserConfigProvider({ children }: { children: ReactNode }) {
  const { t } = useTranslation();
  const { api, ready } = useDeviceApi();
  const { setDirty } = useUnsaved();

  const [soulState, setSoulState] = useState(createAsyncState(""));
  const [userState, setUserState] = useState(createAsyncState(""));
  const [savedSoul, setSavedSoul] = useState("");
  const [savedUser, setSavedUser] = useState("");

  const [soulForm, setSoulForm] = useState<SoulFormState>(defaultSoulForm);
  const [userForm, setUserForm] = useState<UserFormState>(defaultUserForm);

  const [soulSaveStatus, setSoulSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [userSaveStatus, setUserSaveStatus] = useState<
    "idle" | "saving" | "ok" | "fail"
  >("idle");
  const [soulError, setSoulError] = useState("");
  const [userError, setUserError] = useState("");
  const [loadError, setLoadError] = useState("");

  const soulEffective = serializeSoul(soulForm);
  const userEffective = serializeUser(userForm);

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
          const parsed = parseSoul(data);
          setSoulForm(parsed.ok ? parsed.data : defaultSoulForm());
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
          const parsed = parseUser(data);
          setUserForm(parsed.ok ? parsed.data : defaultUserForm());
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
    queueMicrotask(() => {
      loadSoul();
    });
  }, [ready, loadSoul]);

  useEffect(() => {
    if (!ready) return;
    queueMicrotask(() => {
      loadUser();
    });
  }, [ready, loadUser]);

  const handleSaveSoul = useCallback(async () => {
    const payload = serializeSoul(soulForm);
    if (payload.length > MAX_CONTENT) {
      setSoulSaveStatus("fail");
      setSoulError(t("config.validation.contentTooLong"));
      return;
    }
    setSoulSaveStatus("saving");
    setSoulError("");
    const res = await api.soul.save(payload);
    setSoulSaveStatus(res.ok ? "ok" : "fail");
    setSoulError(apiErrorMessage(res.error, t));
    if (res.ok) {
      setSavedSoul(payload);
    }
  }, [api.soul, soulForm, t]);

  const handleSaveUser = useCallback(async () => {
    const payload = serializeUser(userForm);
    if (payload.length > MAX_CONTENT) {
      setUserSaveStatus("fail");
      setUserError(t("config.validation.contentTooLong"));
      return;
    }
    setUserSaveStatus("saving");
    setUserError("");
    const res = await api.user.save(payload);
    setUserSaveStatus(res.ok ? "ok" : "fail");
    setUserError(apiErrorMessage(res.error, t));
    if (res.ok) {
      setSavedUser(payload);
    }
  }, [api.user, userForm, t]);

  useEffect(() => {
    setDirty(soulEffective !== savedSoul || userEffective !== savedUser);
  }, [soulEffective, savedSoul, userEffective, savedUser, setDirty]);

  const dismissSoulSaveFeedback = useCallback(() => {
    setSoulSaveStatus("idle");
    setSoulError("");
  }, []);

  const dismissUserSaveFeedback = useCallback(() => {
    setUserSaveStatus("idle");
    setUserError("");
  }, []);

  const value = useMemo<SoulUserConfigContextValue>(
    () => ({
      ready,
      loadError,
      retryLoad,
      soulForm,
      setSoulForm,
      userForm,
      setUserForm,
      soulState,
      userState,
      soulSaveStatus,
      userSaveStatus,
      soulError,
      userError,
      handleSaveSoul,
      handleSaveUser,
      dismissSoulSaveFeedback,
      dismissUserSaveFeedback,
    }),
    [
      ready,
      loadError,
      retryLoad,
      soulForm,
      userForm,
      soulState,
      userState,
      soulSaveStatus,
      userSaveStatus,
      soulError,
      userError,
      handleSaveSoul,
      handleSaveUser,
      dismissSoulSaveFeedback,
      dismissUserSaveFeedback,
    ],
  );

  return (
    <SoulUserConfigContext.Provider value={value}>
      {children}
    </SoulUserConfigContext.Provider>
  );
}

export function useSoulUserConfig(): SoulUserConfigContextValue {
  const v = useContext(SoulUserConfigContext);
  if (!v) {
    throw new Error("useSoulUserConfig must be used under SoulUserConfigProvider");
  }
  return v;
}
