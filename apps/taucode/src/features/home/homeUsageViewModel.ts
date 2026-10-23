import type {
  LocalUsageDay,
  LocalUsageSnapshot,
} from "../../types";
import { formatRelativeTime } from "../../utils/time";
import {
  formatCompactNumber,
  formatCount,
  formatDayCount,
  formatDayLabel,
  formatDuration,
  formatDurationCompact,
  isUsageDayActive,
} from "./homeFormatters";
import type { HomeStatCard, UsageMetric } from "./homeTypes";

type HomeUsageViewModel = {
  updatedLabel: string | null;
  usageCards: HomeStatCard[];
  usageDays: LocalUsageDay[];
  usageInsights: HomeStatCard[];
};

export function buildHomeUsageViewModel({
  localUsageSnapshot,
  usageMetric,
}: {
  localUsageSnapshot: LocalUsageSnapshot | null;
  usageMetric: UsageMetric;
}): HomeUsageViewModel {
  const usageTotals = localUsageSnapshot?.totals ?? null;
  const usageDays = localUsageSnapshot?.days ?? [];
  const latestUsageDay = usageDays[usageDays.length - 1] ?? null;
  const last7Days = usageDays.slice(-7);
  const last7Tokens = last7Days.reduce((total, day) => total + day.totalTokens, 0);
  const last7Input = last7Days.reduce((total, day) => total + day.inputTokens, 0);
  const last7Cached = last7Days.reduce(
    (total, day) => total + day.cachedInputTokens,
    0,
  );
  const last7AgentMs = last7Days.reduce(
    (total, day) => total + (day.agentTimeMs ?? 0),
    0,
  );
  const last30AgentMs = usageDays.reduce(
    (total, day) => total + (day.agentTimeMs ?? 0),
    0,
  );
  const averageDailyAgentMs =
    last7Days.length > 0 ? Math.round(last7AgentMs / last7Days.length) : 0;
  const last7AgentRuns = last7Days.reduce(
    (total, day) => total + (day.agentRuns ?? 0),
    0,
  );
  const last30AgentRuns = usageDays.reduce(
    (total, day) => total + (day.agentRuns ?? 0),
    0,
  );
  const averageTokensPerRun =
    last7AgentRuns > 0 ? Math.round(last7Tokens / last7AgentRuns) : null;
  const averageRunDurationMs =
    last7AgentRuns > 0 ? Math.round(last7AgentMs / last7AgentRuns) : null;
  const last7ActiveDays = last7Days.filter(isUsageDayActive).length;
  const last30ActiveDays = usageDays.filter(isUsageDayActive).length;
  const averageActiveDayAgentMs =
    last7ActiveDays > 0 ? Math.round(last7AgentMs / last7ActiveDays) : null;
  const peakAgentDay = usageDays.reduce<
    | { day: string; agentTimeMs: number }
    | null
  >((best, day) => {
    const value = day.agentTimeMs ?? 0;
    if (value <= 0) {
      return best;
    }
    if (!best || value > best.agentTimeMs) {
      return { day: day.day, agentTimeMs: value };
    }
    return best;
  }, null);

  let longestStreak = 0;
  let runningStreak = 0;
  for (const day of usageDays) {
    if (isUsageDayActive(day)) {
      runningStreak += 1;
      longestStreak = Math.max(longestStreak, runningStreak);
    } else {
      runningStreak = 0;
    }
  }

  const usageCards: HomeStatCard[] =
    usageMetric === "tokens"
      ? [
          {
            label: "Today",
            value: formatCompactNumber(latestUsageDay?.totalTokens ?? 0),
            suffix: "tokens",
            caption: latestUsageDay
              ? `${formatDayLabel(latestUsageDay.day)} · ${formatCount(
                  latestUsageDay.inputTokens,
                )} in / ${formatCount(latestUsageDay.outputTokens)} out`
              : "Latest available day",
          },
          {
            label: "Last 7 days",
            value: formatCompactNumber(usageTotals?.last7DaysTokens ?? last7Tokens),
            suffix: "tokens",
            caption: `Avg ${formatCompactNumber(usageTotals?.averageDailyTokens)} / day`,
          },
          {
            label: "Last 30 days",
            value: formatCompactNumber(usageTotals?.last30DaysTokens ?? last7Tokens),
            suffix: "tokens",
            caption: `Total ${formatCount(usageTotals?.last30DaysTokens ?? last7Tokens)}`,
          },
          {
            label: "Cache hit rate",
            value: usageTotals
              ? `${usageTotals.cacheHitRatePercent.toFixed(1)}%`
              : "--",
            caption: "Last 7 days",
          },
          {
            label: "Cached tokens",
            value: formatCompactNumber(last7Cached),
            suffix: "saved",
            caption:
              last7Input > 0
                ? `${((last7Cached / last7Input) * 100).toFixed(1)}% of prompt tokens`
                : "Last 7 days",
          },
          {
            label: "Avg / run",
            value:
              averageTokensPerRun === null
                ? "--"
                : formatCompactNumber(averageTokensPerRun),
            suffix: "tokens",
            caption:
              last7AgentRuns > 0
                ? `${formatCount(last7AgentRuns)} runs in last 7 days`
                : "No runs yet",
          },
          {
            label: "Peak day",
            value: formatDayLabel(usageTotals?.peakDay),
            caption: `${formatCompactNumber(usageTotals?.peakDayTokens)} tokens`,
          },
        ]
      : [
          {
            label: "Last 7 days",
            value: formatDurationCompact(last7AgentMs),
            suffix: "agent time",
            caption: `Avg ${formatDurationCompact(averageDailyAgentMs)} / day`,
          },
          {
            label: "Last 30 days",
            value: formatDurationCompact(last30AgentMs),
            suffix: "agent time",
            caption: `Total ${formatDuration(last30AgentMs)}`,
          },
          {
            label: "Runs",
            value: formatCount(last7AgentRuns),
            suffix: "runs",
            caption: `Last 30 days: ${formatCount(last30AgentRuns)} runs`,
          },
          {
            label: "Avg / run",
            value: formatDurationCompact(averageRunDurationMs),
            caption:
              last7AgentRuns > 0
                ? `Across ${formatCount(last7AgentRuns)} runs`
                : "No runs yet",
          },
          {
            label: "Avg / active day",
            value: formatDurationCompact(averageActiveDayAgentMs),
            caption:
              last7ActiveDays > 0
                ? `${formatCount(last7ActiveDays)} active days in last 7`
                : "No active days yet",
          },
          {
            label: "Peak day",
            value: formatDayLabel(peakAgentDay?.day ?? null),
            caption: `${formatDurationCompact(peakAgentDay?.agentTimeMs ?? 0)} agent time`,
          },
        ];

  const usageInsights = [
    {
      label: "Longest streak",
      value: longestStreak > 0 ? formatDayCount(longestStreak) : "--",
      caption:
        longestStreak > 0
          ? "Across current usage range"
          : "No active streak yet",
      compact: true,
    },
    {
      label: "Active days",
      value: last7Days.length > 0 ? `${last7ActiveDays} / ${last7Days.length}` : "--",
      caption:
        usageDays.length > 0
          ? `${last30ActiveDays} / ${usageDays.length} in current range`
          : "No activity yet",
      compact: true,
    },
  ] satisfies HomeStatCard[];

  return {
    updatedLabel: localUsageSnapshot
      ? `Updated ${formatRelativeTime(localUsageSnapshot.updatedAt)}`
      : null,
    usageCards,
    usageDays,
    usageInsights,
  };
}
