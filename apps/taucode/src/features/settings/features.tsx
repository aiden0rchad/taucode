import { createContext, useContext } from "react";
import type { AppSettings } from "@/types";

export const defaultFeatures = {
  git: true,
  worktrees: true,
  browser: true,
  computerUse: true,
  terminal: true,
  sideChats: true,
  subagents: true,
  checkpoints: true,
  chatMode: true,
  workMode: true,
  connectors: true,
  importTools: true,
  dictation: true,
  reasoningEffects: true,
};
export type Features = typeof defaultFeatures;
export function features(settings?: Pick<AppSettings, "features"> | null): Features {
  const value = { ...defaultFeatures, ...settings?.features };
  value.worktrees &&= value.git;
  return value;
}
export const FeatureSettings = createContext<AppSettings | null>(null);
export function useFeatures(): Features { return features(useContext(FeatureSettings)); }
export function useFeatureSettings() { return useContext(FeatureSettings); }
