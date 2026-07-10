import { computed, ref } from "vue";
import * as api from "../api";
import type { StatsSnapshot } from "../core/types";

const emptyStats = (): StatsSnapshot => ({
  totalCopies: 0,
  totalDuplicates: 0,
  textCopies: 0,
  imageCopies: 0,
  todayCopies: 0,
  todayDuplicates: 0,
  historyItems: 0,
  duplicateRate: 0,
  peakHour: null,
  peakHourCount: 0,
  streakDays: 0,
  topRepeats: [],
  daily: [],
});

export function useStats(setStatus: (msg: string, tone?: "muted" | "ok" | "err") => void) {
  const statsOpen = ref(false);
  const stats = ref<StatsSnapshot>(emptyStats());
  const statsLoading = ref(false);

  const duplicateRateLabel = computed(() => {
    const r = stats.value.duplicateRate ?? 0;
    return `${Math.round(r * 100)}%`;
  });

  const peakHourLabel = computed(() => {
    const h = stats.value.peakHour;
    if (h == null) return "—";
    return `${String(h).padStart(2, "0")}:00 · ${stats.value.peakHourCount} 次`;
  });

  const maxDailyTotal = computed(() => {
    let m = stats.value.todayCopies || 1;
    for (const d of stats.value.daily || []) {
      if (d.total > m) m = d.total;
    }
    return Math.max(1, m);
  });

  function dailyBarWidth(total: number) {
    return `${Math.min(100, (total / maxDailyTotal.value) * 100)}%`;
  }

  async function refreshStats() {
    statsLoading.value = true;
    try {
      stats.value = (await api.getFunStats()) || emptyStats();
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error), "err");
    } finally {
      statsLoading.value = false;
    }
  }

  async function openStats() {
    statsOpen.value = true;
    await refreshStats();
  }

  return {
    statsOpen,
    stats,
    statsLoading,
    duplicateRateLabel,
    peakHourLabel,
    dailyBarWidth,
    openStats,
    refreshStats,
  };
}
