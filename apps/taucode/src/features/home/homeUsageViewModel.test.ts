import { describe, expect, it } from "vitest";
import type { LocalUsageSnapshot } from "../../types";
import { buildHomeUsageViewModel } from "./homeUsageViewModel";

const snapshot: LocalUsageSnapshot = {
  updatedAt: Date.now(),
  days: [],
  topModels: [],
  totals: {
    allTimeTokens: 123456,
    allTimeAgentRuns: 42,
    allTimeAgentTimeMs: 3_600_000,
    last7DaysTokens: 0,
    last30DaysTokens: 0,
    averageDailyTokens: 0,
    cacheHitRatePercent: 0,
    peakDay: null,
    peakDayTokens: 0,
  },
};

describe("persistent usage totals", () => {
  it("shows lifetime usage even when there is no usage in the chart range", () => {
    const { usageCards } = buildHomeUsageViewModel({ localUsageSnapshot: snapshot, usageMetric: "tokens" });
    expect(usageCards.find(card => card.label === "All time")?.value).toBe("123k");
    expect(usageCards.find(card => card.label === "Last 30 days")?.value).toBe("0");
  });

  it("uses lifetime response counts and estimated time in the time view", () => {
    const { usageCards } = buildHomeUsageViewModel({ localUsageSnapshot: snapshot, usageMetric: "time" });
    expect(usageCards.find(card => card.label === "All time")).toMatchObject({
      value: "1.0h", caption: "42 recorded model responses · estimated time",
    });
  });
});
