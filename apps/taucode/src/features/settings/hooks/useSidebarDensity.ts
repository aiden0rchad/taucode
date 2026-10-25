import { useEffect, useSyncExternalStore } from "react";

const key = "taucode.sidebar-density";
const event = "taucode:sidebar-density";
const read = () => localStorage.getItem(key) === "comfortable" ? "comfortable" : "compact";
const subscribe = (notify: () => void) => {
  window.addEventListener(event, notify);
  window.addEventListener("storage", notify);
  return () => { window.removeEventListener(event, notify); window.removeEventListener("storage", notify); };
};
export function useSidebarDensity() {
  const density = useSyncExternalStore(subscribe, read, () => "compact");
  useEffect(() => { document.documentElement.dataset.sidebarDensity = density; }, [density]);
  return [density, (value: string) => {
    localStorage.setItem(key, value === "comfortable" ? "comfortable" : "compact");
    window.dispatchEvent(new Event(event));
  }] as const;
}
