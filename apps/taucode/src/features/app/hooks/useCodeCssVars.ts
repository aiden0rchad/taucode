import { useEffect } from "react";
import type { AppSettings } from "../../../types";

export function useCodeCssVars(appSettings: AppSettings) {
  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }
    const root = document.documentElement;
    root.style.setProperty("--ui-font-family", appSettings.uiFontFamily);
    root.style.setProperty("--code-font-family", appSettings.codeFontFamily);
    root.style.setProperty("--code-font-size", `${appSettings.codeFontSize}px`);
  }, [appSettings.uiFontFamily, appSettings.codeFontFamily, appSettings.codeFontSize]);
}

