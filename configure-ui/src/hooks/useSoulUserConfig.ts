import { useContext } from "react";
import {
  SoulUserConfigContext,
  type SoulUserConfigContextValue,
} from "../contexts/SoulUserConfigContext";

export function useSoulUserConfig(): SoulUserConfigContextValue {
  const v = useContext(SoulUserConfigContext);
  if (!v) {
    throw new Error("useSoulUserConfig must be used under SoulUserConfigProvider");
  }
  return v;
}
